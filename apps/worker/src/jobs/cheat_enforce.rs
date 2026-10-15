//! 作弊事件累进处置（2026-10-07 保种组实测审计 P1-5）。
//!
//! 背景：cheat_events（over_ceiling / counter_reset / ghost_seed / torrent_gap）
//! 旧版「只记不罚」——实测刷 1.32 TiB 仅得 3 行 hits=1，作恶零成本、管理端
//! 默认无人看。本 job 补执法闭环的自动化分级：
//!   L1（首次）   ：站内信告知用户存在异常流量记录（教育优先，PT 站惯例）
//!   L2（≥5 hits）：PM 管理组信箱置顶（staffmessages，permission='cheater'）
//!                  + webhook 广播（复用 audit::webhook_broadcast）
//!                  + 自动开案（0367 M1：reports ref_type='cheat' 落案件，
//!                    同用户至多一个进行中案件，署名取在册最低 staff）
//!   （经济侧拉黑由 seeding_reward 的 NOT EXISTS 承担：存在未处置
//!     ghost/speed/reset 事件的用户停发做种收益，管理组处置后自动恢复）
//!
//! 幂等：每用户每事件只发一次 L1/L2——L1 靠 messages 里带 `[[cheat:{agent}]]`
//! 隐式键的 7 天 NOT EXISTS（六轮补齐，此前 L1 无去重、实测 10 分钟一刷）；
//! L2 靠 staffmessages 的 NOT EXISTS（管理组处置 resolved_at 置位后同
//! 一事件不再重复告警）。员工（class_id ≥ 90）不自动发信，避免测试/运维
//! 流量误伤。
//!
//! 自动开案的幂等与告警去重**相互独立**：告警 7 天窗口挡重，案件靠
//! uq_cheat_case_open 唯一索引挡重——告警可能已发过，但案件被 done 关闭后
//! 新证据再达 L2 线应开新案（ON CONFLICT 只挡 status IN (0,2) 的进行中案件）。

use super::sanction_sweep::sanction_sweep;
use super::staff_alert::{alerted_within_7d, staff_alert};
use sqlx::PgPool;

const L2_SUBJ: &str = "作弊事件累进告警";

/// 案件 SLA（0367 M1）：进行中案件超过此时长未认领/处理 → sla_breached。
/// 与 api 侧 cheat_case::SLA_HOURS 同值（worker 不便引 api crate，
/// 改动时两处同步；这是策略常量，调它本来就要发版）。
const CASE_SLA_HOURS: i64 = 72;

/// 自动开案（0367 M1 起 L2 用；0368 M2 风险分 high/critical 档复用）。
/// 同用户至多一个进行中案件（uq_cheat_case_open）——ON CONFLICT 命中返回
/// None（含「此前案件被 done 关闭后开新案」由索引谓词 status IN (0,2) 放行）。
/// 署名取在册最低 staff（与 staff_alert 同口径：墓碑号会被 user_del 的清理
/// 级联带走）；无在册职员时 SELECT 出 0 行，静默返回 None。
/// `note_head` 为首条备注前缀，归案证据数由本函数实测后追加。
pub(crate) async fn auto_open_case(
    db: &PgPool,
    uid: i64,
    reason: &str,
    note_head: &str,
    priority: i16,
) -> Option<i64> {
    // risk_score 取当前风险分（0368：开案即固化当时的分数快照）
    // ? 语义：ON CONFLICT 命中（已有进行中案件）或无在册职员 → 返回 None，
    // 由调用方决定是否重试/告警；SQL 报错同样吞掉返回 None。
    let case_id: i64 = sqlx::query_scalar(
        "INSERT INTO reports \
           (reporter_id, ref_type, ref_id, reason, priority, risk_score) \
         SELECT s.uid, 'cheat', $1, $2, $3, \
                (SELECT score FROM user_risk_scores WHERE user_id = $1) \
         FROM (SELECT MIN(id) AS uid FROM users \
                WHERE class_id >= 90 AND status < 2) s \
         WHERE s.uid IS NOT NULL \
         ON CONFLICT (ref_id) \
           WHERE ref_type = 'cheat' AND status IN (0, 2) \
         DO NOTHING RETURNING id",
    )
    .bind(uid)
    .bind(reason)
    .bind(priority)
    .fetch_optional(db)
    .await
    .unwrap_or_else(|e| {
        tracing::warn!(%e, user = uid, "auto_open_case: 开案失败");
        None
    })?;
    // 证据归案：该用户全部游离未处置证据挂到本案
    let n = sqlx::query(
        "UPDATE cheat_events SET case_id = $1 \
         WHERE user_id = $2 AND resolved_at IS NULL AND case_id IS NULL",
    )
    .bind(case_id)
    .bind(uid)
    .execute(db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0);
    // 首条备注（作者同署名 staff；与上条 INSERT 同 tick，MIN(id) 职员理论上
    // 可能变，出 0 行时不插不报）
    let _ = sqlx::query(
        "INSERT INTO case_notes (case_id, author_id, body) \
         SELECT $1, s.uid, $2 \
         FROM (SELECT MIN(id) AS uid FROM users \
                WHERE class_id >= 90 AND status < 2) s",
    )
    .bind(case_id)
    .bind(format!("{note_head}；归案未处置证据 {n} 条。请及时认领核实并处置。"))
    .execute(db)
    .await;
    Some(case_id)
}

pub(crate) async fn cheat_enforce(db: &PgPool) -> anyhow::Result<u64> {
    // 案件 SLA 置位（0367）：每 tick 一把 UPDATE，幂等（NOT sla_breached）。
    // 进行中 = status IN (0,2)（待处理/已认领）；done 的不再计时。
    // 注意 make_interval 的 hours 参数是 int4，Rust i64 绑成 int8 无隐式转换
    // ⇒ 必须 ::int 强转（同 user_grant_medal/user_revoke 惯例；不转整个 job
    // 每分钟报 function make_interval(hours => bigint) does not exist 挂掉）。
    let breached = sqlx::query(
        "UPDATE reports SET sla_breached = TRUE \
         WHERE ref_type = 'cheat' AND status IN (0, 2) AND NOT sla_breached \
           AND created_at < now() - make_interval(hours => $1::int)",
    )
    .bind(CASE_SLA_HOURS)
    .execute(db)
    .await?
    .rows_affected();
    if breached > 0 {
        tracing::warn!(breached, "cheat_enforce: 案件 SLA 超时置位");
    }

    // 停权到期清扫（0369 M3）：逻辑在 sanction_sweep.rs（本 job 每 10m 调一次）。
    sanction_sweep(db).await?;

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
            // 去重要读出来判：旧写法把去重塞进 INSERT 的 NOT EXISTS，命中时
            // 聚合返回一行 NULL 署名 ⇒ 撞 user_id NOT NULL，"不发"变成报错
            let prefix = format!("[[cheat:{agent}]]用户 #{uid} ");
            let body = format!(
                "{prefix}的作弊事件累计达 {hits} 次（{agent} / {reason}），\
                 其做种收益已被自动暂停，并已在后台「反作弊案件」自动开案。\
                 请及时认领核实并处置。"
            );
            let dup =
                alerted_within_7d(db, "cheater", L2_SUBJ, &prefix).await;
            let n = if dup {
                0
            } else {
                staff_alert(db, L2_SUBJ, &body, "cheater").await
            };
            if n > 0 {
                tracing::warn!(
                    user = uid,
                    %agent,
                    hits,
                    "cheat_enforce: L2 告警"
                );
            }
            // L2 自动开案（0367 M1）：确保该用户存在进行中案件。
            // 与 7 天告警去重相互独立：告警挡过但案件 done 后新证据再达 L2
            // 线会开新案（唯一索引只挡 status IN (0,2)）。
            if let Some(cid) = auto_open_case(
                db,
                uid,
                &format!("自动开案：{agent} 累计 {hits} 次（{reason}）"),
                &format!("系统自动开案：{agent} 累计 {hits} 次达 L2 线"),
                1,
            )
            .await
            {
                tracing::warn!(user = uid, case = cid, %agent, hits,
                    "cheat_enforce: L2 自动开案");
            }
        }
    }
    Ok(acted)
}
