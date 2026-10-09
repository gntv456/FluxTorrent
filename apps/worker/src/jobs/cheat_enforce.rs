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
//! 幂等：每用户每事件只发一次 L1/L2——L1 靠 messages 里带 `[[cheat:{agent}]]`
//! 隐式键的 7 天 NOT EXISTS（六轮补齐，此前 L1 无去重、实测 10 分钟一刷）；
//! L2 靠 staffmessages 的 NOT EXISTS（管理组处置 resolved_at 置位后同
//! 一事件不再重复告警）。员工（class_id ≥ 90）不自动发信，避免测试/运维
//! 流量误伤。

use sqlx::PgPool;

pub async fn cheat_enforce(db: &PgPool) -> anyhow::Result<u64> {
    // 高危类：进 L1/L2 告警网的事件。
    // 五轮实测两处断口（2026-10-08）：
    //  ① 旧写法 `c.agent = 'torrent:%'` 是**等值**比较，字面量里带 %——
    //     永远匹配不到任何行（audit.rs 写的是 `torrent:{id}`）。应为 LIKE。
    //  ② `corr:`（自报上传超佐证上界）、`xreport_over:`（佐证量超出本人物理
    //     额度）、`xreport:absurd`（单次佐证超 10 GiB）、`nearcap:`（0304 贴边
    //     汇报画像）这四类此前**不在任何处置过滤器里**：写了台账、面板看得见，
    //     但 L1/L2 永不触发——0304 那套「网盘挂 NAS 临时挂载型假做种专杀」
    //     实测就是只产画像不产处置。
    // 注意本表与 seeding_reward 的「做种收益拉黑名单」（ghost/speed/reset 三类）
    // 是**有意的两个口径**：告警面宽于经济拉黑面。seeding.rs 此刻是另一路的
    // 在制品，扩它那份名单留到下一批（见本轮报告「遗留」）。
    let rows: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT c.user_id, c.agent, c.reason, c.hits \
         FROM cheat_events c \
         JOIN users u ON u.id = c.user_id \
         WHERE c.resolved_at IS NULL AND c.hits >= 1 \
           AND (c.agent LIKE 'ghost:%' OR c.agent LIKE 'speed:%' \
                OR c.agent LIKE 'reset:%' OR c.agent LIKE 'torrent:%' \
                OR c.agent LIKE 'corr:%' \
                OR c.agent LIKE 'xreport_over:%' \
                OR c.agent LIKE 'nearcap:%' \
                OR c.agent LIKE 'deal:%' \
                OR c.agent = 'xreport:absurd' \
                OR c.agent = 'down_under' \
                OR c.agent = 'bitthief') \
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

    // 累进告警线（0311 site_settings 化）：开源后出厂值人人可查，
    // 各站按社区规模自调。缺行时回落出厂值 3/5。
    let l1_hits: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'cheat_l1_hits'), 3)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(3);
    let l2_hits: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'cheat_l2_hits'), 5)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(5);
    let l2_hits = l2_hits.max(l1_hits); // 防倒挂：L2 不低于 L1

    let mut acted = 0u64;
    for (uid, agent, reason, hits) in rows {
        // L1：用户告知。六轮审计 P2-E 修两件：
        //  ① 阈值 hits>=1 太低——任何一条低置信信号（贴边节奏/单次超窗）
        //    就发信，且每轮（10 分钟）重发：实测用户 39 收 50 条同文提醒。
        //    提到 L1 线（默认 3；L1 是教育性告知，不是处罚）。
        //  ② 去重——文档声称的「每用户×事件只发一次」此前只对 L2 生效
        //    （staffmessages 靠 NOT EXISTS resolved 挡重），L1 无任何去重。
        //    与 L2 同源：该 (user, agent) 存在**已发出且对应事件未处置**
        //    的提醒即跳过——用 messages 的 subject + body 前缀匹配（messages
        //    无 (receiver, kind) 唯一键，不能上硬约束）。
        if warn_action && hits >= l1_hits {
            let n = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 SELECT NULL, $1, '流量记录异常提醒', $2 \
                 FROM users u \
                 WHERE u.id = $1 AND u.class_id < 90 \
                   AND NOT EXISTS ( \
                     SELECT 1 FROM messages m \
                     WHERE m.receiver_id = $1 \
                       AND m.subject = '流量记录异常提醒' \
                       AND m.body LIKE $3 || '%' \
                       AND m.created_at > now() - interval '7 days')",
            )
            .bind(uid)
            .bind(format!(
                "[[cheat:{agent}]]系统检测到您的做种/流量数据存在异常记录（{}，累计 {} 次）。\
                 若您确在使用特殊客户端或代理，请联系管理组说明；未处理的异常记录\
                 将暂停做种收益结算，管理组核实后自动恢复。",
                reason, hits
            ))
            .bind(format!("[[cheat:{agent}]]"))
            .execute(db)
            .await
            .map(|r| r.rows_affected())
            .unwrap_or(0);
            if n > 0 {
                acted += 1;
            }
        }
        // L2：管理组信箱（首次命中即报，同一 (user, agent) 7 天去重）。
        // 去重方向（六轮同款教训）：挡的是**7 天内已告警过**，不是「处置过
        // 就永久静默」——旧写法 NOT EXISTS resolved 会两个方向都错：
        // ① 未处置事件每 10 分钟重刷管理组信箱；② 处置后 hits 再涨也永不
        // 告警。改按 staffmessages 侧 7 天窗口去重（body 前缀带 user+agent）。
        if hits >= l2_hits {
            let n = sqlx::query(
                "INSERT INTO staffmessages \
                 (user_id, subject, body, permission) \
                 SELECT MIN(id), '作弊事件累进告警', $1, 'cheater' \
                 FROM users WHERE class_id >= 90 \
                   AND NOT EXISTS ( \
                     SELECT 1 FROM staffmessages sm \
                     WHERE sm.permission = 'cheater' \
                       AND sm.subject = '作弊事件累进告警' \
                       AND sm.body LIKE $2 || '%' \
                       AND sm.created_at > now() - interval '7 days')",
            )
            .bind(format!(
                "[[cheat:{agent}]]用户 #{} 的作弊事件累计达 {hits} 次（{agent} / \
                 {reason}），其做种收益已被自动暂停。\
                 请在后台「作弊探测」核实并处置。",
                uid
            ))
            .bind(format!("[[cheat:{agent}]]用户 #{uid} "))
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
