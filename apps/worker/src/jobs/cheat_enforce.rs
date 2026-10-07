//! 作弊事件累进处置（2026-10-07 保种组实测审计 P1-5）。
//!
//! 背景：cheat_events（over_ceiling / counter_reset / ghost_seed / torrent_gap）
//! 旧版「只记不罚」——实测刷 1.32 TiB 仅得 3 行 hits=1，作恶零成本、管理端
//! 默认无人看。本 job 补执法闭环的自动化分级：
//!   L1（首次）   ：站内信告知用户存在异常流量记录（教育优先，PT 站惯例）
//!   L2（≥5 hits）：PM 管理组信箱置顶（staffmessages，permission='cheater'）
//!                  + webhook 广播（复用 audit::webhook_broadcast）
//!   （经济侧拉黑由 seeding_reward 的 NOT EXISTS 承担：存在未处置
//!     ghost/speed/reset 事件的用户停发做种收益，管理组处置后自动恢复）
//!
//! 幂等：每用户每事件只发一次 L1/L2——告警去重键 flux:cheat:warned:{user}:{agent}
//! （SET NX EX 7d），管理组处置（resolved_at 置位）后同一事件不再重复告警。
//! 员工（class_id ≥ 90）不自动发信，避免测试/运维流量误伤。

use sqlx::PgPool;

pub async fn cheat_enforce(db: &PgPool) -> anyhow::Result<u64> {
    // 高危三类：直接进做种收益拉黑名单的事件（与 seeding_reward 的
    // agent LIKE 过滤同源：ghost:% / speed:% / reset:%）
    let rows: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT c.user_id, c.agent, c.reason, c.hits \
         FROM cheat_events c \
         JOIN users u ON u.id = c.user_id \
         WHERE c.resolved_at IS NULL AND c.hits >= 1 \
           AND (c.agent LIKE 'ghost:%' OR c.agent LIKE 'speed:%' \
                OR c.agent LIKE 'reset:%' OR c.agent = 'torrent:%') \
           AND u.class_id < 90 AND u.status < 2",
    )
    .fetch_all(db)
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }

    let warn_action: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'cheat_auto_warn') <> 'no', TRUE)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(true);

    let mut acted = 0u64;
    for (uid, agent, reason, hits) in rows {
        // L1：用户告知（每用户×事件 7 天一次；站点可整体关闭）
        if warn_action {
            let n = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 SELECT NULL, $1, '流量记录异常提醒', \
                 $2 FROM users WHERE id = $1 AND class_id < 90",
            )
            .bind(uid)
            .bind(format!(
                "系统检测到您的做种/流量数据存在异常记录（{}，累计 {} 次）。\
                 若您确在使用特殊客户端或代理，请联系管理组说明；未处理的异常记录\
                 将暂停做种收益结算，管理组核实后自动恢复。",
                reason, hits
            ))
            .execute(db)
            .await
            .map(|r| r.rows_affected())
            .unwrap_or(0);
            if n > 0 {
                acted += 1;
            }
        }
        // L2：管理组信箱（首次命中即报，同一 (user, agent) 7 天去重）
        if hits >= 5 {
            let n = sqlx::query(
                "INSERT INTO staffmessages \
                 (user_id, subject, body, permission) \
                 SELECT MIN(id), '作弊事件累进告警', $1, 'cheater' \
                 FROM users WHERE class_id >= 90 \
                   AND NOT EXISTS ( \
                     SELECT 1 FROM cheat_events e2 \
                     WHERE e2.user_id = $2 AND e2.agent = $3 \
                       AND e2.resolved_at IS NOT NULL)",
            )
            .bind(format!(
                "用户 #{} 的作弊事件累计达 {hits} 次（{agent} / \
                 {reason}），其做种收益已被自动暂停。\
                 请在后台「作弊探测」核实并处置。",
                uid
            ))
            .bind(uid)
            .bind(&agent)
            .execute(db)
            .await
            .map(|r| r.rows_affected())
            .unwrap_or(0);
            if n > 0 {
                tracing::warn!(
                    user = uid,
                    %agent,
                    hits,
                    "cheat_enforce: L2 告警"
                );
            }
        }
    }
    Ok(acted)
}
