//! harem 邀请持续分润（0354）：做种结算的邀请人分成。
//! 从 seeding.rs 拆出（300 行门禁）；口径与迁移 0354 注释互为镜像。
//!
//! 公式（ColaPT harem 的参数化对应，pct=`seeding_referral_pct`）：
//!   每个邀请人每小时分润 = LEAST( Σ(被邀请人 amount) × pct%, 邀请人自身 amount )
//! 即「分润不超过邀请人自身产出」。pct=20 时与 ColaPT 原式
//! min(own/0.2, Σinvitee) × 0.2 **代数完全一致**：
//!   min(own/0.2, Σ)×0.2 = min(own, Σ×0.2)（两边同乘 0.2，min 保序）。
//! kind='seeding_referral'（独立于 seeding_reward——game_coupons 按
//! kind='seeding_reward' 判定每日领券，混入会污染口径）；幂等键
//! `seeding-inv:{inviter}:{yyyymmddhh}`；pct=0 时 WHERE 短路不产生行。

use sqlx::PgPool;

/// 分润钳制口径的 Rust 镜像（**单测口径锁定用**，结算本体在上文 SQL）。
/// 代数等价变形（pct=20 即 ColaPT 原式 min(own/0.2, Σ)×0.2 = min(own, Σ×0.2)）：
///   bonus = LEAST( Σ(被邀请人 amount) × pct% , 邀请人自身 amount )
/// 即「分润不超过邀请人自身产出」；整除截断与 SQL 的 numeric→bigint 同向。
fn referral_bonus(own: i64, invitees_total: i64, pct: i64) -> i64 {
    if pct <= 0 {
        return 0;
    }
    (invitees_total * pct / 100).min(own)
}

/// 邀请分润结算（紧随主结算的独立语句；余额由 seeding.rs 的
/// 「近 2 小时有流水」重算段覆盖——重算段按 user_id 聚合不筛 kind，
/// 新 kind 的行天然进窗口，无需放宽条件）。
pub(super) async fn seeding_referral(
    db: &PgPool,
    hour: &str,
) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        -- 与主结算同一小时参数（$1 = yyyymmddhh）。只对「本小时已落了
        -- seeding_reward 流水」的用户结算（earned 子查询），保证两边窗口
        -- 一致：被邀请人本 hour 没产出 ⇒ 不构成分润；邀请人本 hour 没产出
        -- ⇒ 钳制为 0。pct 从 site_settings 内联读（改 seeding_params()
        -- 返回列会动到 API 预估共用面，侵入更大，故选内联）。
        INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
        SELECT nextval('spark_ledger_id_seq'), r.inviter, r.bonus,
               'seeding_referral', 'seeding-inv:' || r.inviter || ':' || $1
        FROM (
            WITH earned AS (
                SELECT user_id, amount FROM spark_ledger
                WHERE kind = 'seeding_reward'
                  AND idempotency_key = 'seeding:' || user_id || ':' || $1
            ),
            pct AS (
                SELECT COALESCE(
                    (SELECT value::numeric FROM site_settings
                     WHERE name = 'seeding_referral_pct'), 20
                ) AS pct
            )
            SELECT e_inv.user_id AS inviter,
                   LEAST(
                       e_own.amount,
                       (SUM(e_inv.amount) * pct.pct / 100)
                   )::bigint AS bonus
            FROM users u
            JOIN earned e_inv ON e_inv.user_id = u.id
            JOIN users inv ON inv.id = u.invited_by
            JOIN earned e_own ON e_own.user_id = inv.id
            CROSS JOIN pct pct
            WHERE u.status < 2 AND inv.status < 2
              AND pct.pct > 0
            GROUP BY e_inv.user_id, e_own.amount, pct.pct
        ) r
        WHERE r.bonus > 0
          AND NOT EXISTS (
              SELECT 1 FROM spark_ledger l
              WHERE l.idempotency_key =
                    'seeding-inv:' || r.inviter || ':' || $1
          )
        "#,
    )
    .bind(hour)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::referral_bonus;

    /// pct=0（关闭）恒为 0——SQL 侧同款短路（pct.pct > 0 不产生行）
    #[test]
    fn referral_zero_pct_yields_nothing() {
        assert_eq!(referral_bonus(1000, 1000, 0), 0);
        assert_eq!(referral_bonus(1000, 1000, -1), 0);
    }

    /// 邀请人自身 0 产出 → 分润 0（纯挂名不劳而获被钳）
    #[test]
    fn referral_zero_own_output_clamped_to_zero() {
        assert_eq!(referral_bonus(0, 10_000, 20), 0);
    }

    /// 正常区（未触钳）：Σinvitee × pct%
    #[test]
    fn referral_normal_share_below_cap() {
        // 自身 1000；被邀合计 300×20% = 60 < 1000（不触钳）
        assert_eq!(referral_bonus(1000, 300, 20), 60);
    }

    /// 被邀合计超自身产出 → 被钳到自身产出（ColaPT 口径 pct=20 对齐）
    #[test]
    fn referral_clamped_to_own_proportional_cap() {
        // 自身 100；被邀合计 10000×20% = 2000 > 100 ⇒ 钳到 100
        assert_eq!(referral_bonus(100, 10_000, 20), 100);
        // ColaPT 原式代数等价锁定：min(own/0.2, Σ)×0.2 = min(own, Σ×0.2)
        let cola = (100_f64 / 0.2).min(10_000_f64) * 0.2;
        assert!((100.0 - cola).abs() < 1e-9);
    }
}
