//! 自动审种·管理摘要与队列健康（2026-11-28 从 auto_review.rs 拆出守行数门）。
//!
//! - `auto_review_digest`（§4.3）：昨日执法四指标日报，站内信发管理组，一天一封
//! - `queue_health`（§13.4/§15.1）：待审队列深度/滞留观测超时升级，一天最多一封
//!
//! 二者被 auto_review::auto_review_tick 与 manual 面板消费；SQL 口径与
//! api 侧 review_auto 同源（共享表台账，幂等设计双跑不重复计数）。
use sqlx::PgPool;

/// 管理摘要日报（§4.3）：昨日自动通过/拒绝/暂缓 + 影子四指标 + Top 规则。
/// 幂等锚：站内信 kind=review_auto_digest 当日只发一封（messages 查重）。
pub async fn auto_review_digest(db: &PgPool) -> anyhow::Result<()> {
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value = '1' FROM site_settings          WHERE name = 'auto_review_notify_admin'), true)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(true);
    if !enabled {
        return Ok(());
    }
    // 今日已发则跳过
    let sent: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages WHERE kind = 'review_auto_digest' \
         AND created_at > \
         (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') \
         AT TIME ZONE 'Asia/Shanghai')",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    if sent > 0 {
        return Ok(());
    }
    let (pass, deny, postpone): (i64, i64, i64) = sqlx::query_as(
        "SELECT             count(*) FILTER (WHERE verdict = 0),             count(*) FILTER (WHERE verdict = 1),             count(*) FILTER (WHERE verdict = 2)          FROM auto_review_log          WHERE decided_by = 0            AND created_at >= (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') - interval '1 day') AT TIME ZONE 'Asia/Shanghai'            AND created_at < (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai')) AT TIME ZONE 'Asia/Shanghai'",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or((0, 0, 0));
    let (agree, total): (i64, i64) = sqlx::query_as(
        "SELECT             count(*) FILTER (WHERE                 (l.verdict = 0 AND t.approval_status = 1) OR                 (l.verdict = 1 AND t.approval_status = 2)),             count(*)          FROM auto_review_log l JOIN torrents t ON t.id = l.torrent_id          WHERE l.decided_by = 0 AND t.approval_status IN (1, 2)",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or((0, 0));
    let agree_pct = if total > 0 { agree * 100 / total } else { 0 };
    let top_rules: Vec<(String, i64)> = sqlx::query_as(
        "SELECT e->>'rule' AS rule, count(*)          FROM auto_review_log l, jsonb_array_elements(l.hit_rules) e          WHERE l.created_at >= (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai') - interval '1 day') AT TIME ZONE 'Asia/Shanghai'            AND l.created_at < (date_trunc('day', now() AT TIME ZONE 'Asia/Shanghai')) AT TIME ZONE 'Asia/Shanghai'          GROUP BY 1 ORDER BY 2 DESC LIMIT 5",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let top = top_rules
        .iter()
        .map(|(r, n)| format!("{r}×{n}"))
        .collect::<Vec<_>>()
        .join("、");
    let subject = "自动审种每日摘要".to_string();
    let body = format!(
        "昨日：自动通过 {pass} / 自动拒绝 {deny} / 暂缓 {postpone}。
         影子一致率 {agree_pct}%（{agree}/{total} 样本）。
         Top 规则：{top}。详情见管理台「自动审种」。"
    );
    // 发给管理组：逐个 class≥99 在册用户（messages 无组播，逐条插）
    let staff: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE class_id >= 99 AND status < 2 LIMIT 20",
    )
    .bind(0)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    for uid in staff {
        let _ = sqlx::query(
            "INSERT INTO messages (receiver_id, subject, body, kind, params)              SELECT $1, $2, $3, 'review_auto_digest', '{}'::jsonb              WHERE EXISTS (SELECT 1 FROM users WHERE id = $1)",
        )
        .bind(uid)
        .bind(&subject)
        .bind(&body)
        .execute(db)
        .await;
    }
    Ok(())
}

/// §13.4 + §15.1：队列健康观测（超时升级 + 深度告警），均一天最多一封。
pub(crate) async fn queue_health(db: &PgPool) {
    let (depth, stale): (i64, i64) = sqlx::query_as(
        "SELECT             (SELECT count(*) FROM torrents WHERE approval_status = 0),             (SELECT count(*) FROM torrents t WHERE t.approval_status = 0                AND t.mtime < now() - make_interval(hours => 24)                AND NOT EXISTS (SELECT 1 FROM auto_review_log l                      WHERE l.torrent_id = t.id AND l.decided_by = 0))",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or((0, 0));
    let alerted_today: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM staffmessages          WHERE subject LIKE '自动审种：队列%'            AND created_at > now() - interval '20 hours'",
    )
    .bind(0)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    if alerted_today > 0 {
        return;
    }
    let msg = if stale > 0 {
        Some(format!(
            "待审队列有 {stale} 个种子超过 24 小时无人处理（且未经机器评估）。             请审核组关注：管理台 → 种子审核。"
        ))
    } else if depth > 50 {
        Some(format!(
            "待审队列深度 {depth}（>50）。自动审种引擎在持续消化，             人工侧可帮忙看暂缓件：管理台 → 自动审种。"
        ))
    } else {
        None
    };
    if let Some(body) = msg {
        // 走 worker 的管理组告警**唯一出口**（staff_alert_as，署名档 99
        // = 与 leak_bot 同口径）：守卫（无在册署名不插行、只署在册号）
        // 收在出口件里，闸门 BQ 的 R1d 才站得住。
        super::staff_alert::staff_alert_as(
            db,
            "自动审种：队列健康提醒",
            &body,
            "torrent",
            99,
        )
        .await;
    }
}
