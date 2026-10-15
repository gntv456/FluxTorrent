//! 作弊停权到期清扫（0369 M3）。
//! 从 cheat_enforce.rs 拆出（300 行门禁），由该 job 每 10m 调用一次。
//!
//! 三件事（全幂等，均为集合 UPDATE）：
//!   ① users.cheat_banned_until 过期 → 置 NULL，恢复下载/做种收益资格
//!      （ensure_downloadable 与 seeding_reward 同读此列）。
//!   ② reports.sanction 中已 applied 且到期的 ban/full_ban → state=expired 留痕。
//!   ③ users 中过期的申诉令牌 → 作废（用后即焚 + 到期作废双保险）。
//!
//! 注意：full_ban 另置的 users.status=2 **不**随停权到期自动恢复——全站封禁
//! 须人工复核或申诉 overturned（与 api approve.rs 文档口径一致）。

use sqlx::PgPool;

pub(crate) async fn sanction_sweep(db: &PgPool) -> anyhow::Result<u64> {
    let mut acted = 0u64;
    let unbanned = sqlx::query(
        "UPDATE users SET cheat_banned_until = NULL \
         WHERE cheat_banned_until IS NOT NULL AND cheat_banned_until <= now()",
    )
    .execute(db)
    .await?
    .rows_affected();
    if unbanned > 0 {
        tracing::warn!(unbanned, "sanction_sweep: 停权到期自动解除");
        acted += unbanned;
    }
    // 顶层键浅合并（`||`）：只置换 state，其余字段保留。比 jsonb_set 链式
    // 更直白，也避开 0328 备忘里「父路径不存在不建中间层」的坑。
    let expired = sqlx::query(
        "UPDATE reports SET \
           sanction = sanction || '{\"state\":\"expired\"}'::jsonb \
         WHERE ref_type = 'cheat' AND sanction->>'state' = 'applied' \
           AND sanction->>'kind' IN ('ban', 'full_ban') \
           AND sanction ? 'applied_at' \
           AND (sanction->>'applied_at')::timestamptz \
               + make_interval(days => (sanction->>'ban_days')::int) <= now()",
    )
    .execute(db)
    .await?
    .rows_affected();
    if expired > 0 {
        tracing::info!(expired, "sanction_sweep: 停权处置到期置 expired");
        acted += expired;
    }
    let tok = sqlx::query(
        "UPDATE users SET appeal_token_hash = NULL, \
           appeal_token_expires_at = NULL \
         WHERE appeal_token_expires_at IS NOT NULL \
           AND appeal_token_expires_at <= now()",
    )
    .execute(db)
    .await?
    .rows_affected();
    if tok > 0 {
        tracing::info!(tok, "sanction_sweep: 申诉令牌到期作废");
        acted += tok;
    }
    Ok(acted)
}
