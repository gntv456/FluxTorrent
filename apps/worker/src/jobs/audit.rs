//! 作弊检测（cheat_audit / multi_ip_check）+ Ratio Watch。

use sqlx::PgPool;

/// 0228 运维 webhook 广播（worker 侧：直接查 site_settings + reqwest，
/// 失败只落日志）
async fn webhook_broadcast(db: &PgPool, text: &str) {
    let keys: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN          ('webhook_discord', 'tg_bot_token', 'tg_chat_id')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let kv: std::collections::HashMap<_, _> = keys.into_iter().collect();
    let client = reqwest::Client::new();
    if let Some(url) = kv.get("webhook_discord").filter(|u| !u.is_empty()) {
        let _ = client
            .post(url)
            .json(&serde_json::json!({ "content": text }))
            .send()
            .await;
    }
    if let (Some(token), Some(chat)) = (
        kv.get("tg_bot_token").filter(|t| !t.is_empty()),
        kv.get("tg_chat_id").filter(|c| !c.is_empty()),
    ) {
        let _ = client
            .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
            .json(&serde_json::json!({
                "chat_id": chat, "text": text,
            }))
            .send()
            .await;
    }
}

/// 种子级 up/down 差额对账（NP cheaterbox 口径）：
/// 虚报上传者没有对应真实下载方，同种子 7 天窗口 Σ(delta_up) − Σ(delta_down) 长期为正且巨大。
/// 免费促销（free/x2free，含全局/官种/分类维度）天然产生差额，豁免。
/// 命中 → cheat_events（agent='torrent_gap', agent 字段存 torrent:{id}）+ 首次进管理组信箱。
pub async fn cheat_audit(db: &PgPool) -> anyhow::Result<u64> {
    let threshold_gb: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'cheat_gap_threshold_gb'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(50)
    .clamp(1, 10240);
    let threshold = threshold_gb * 1024 * 1024 * 1024;

    let rows: Vec<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT torrent_id, SUM(delta_up) - SUM(delta_down) AS gap
        FROM traffic_ledger
        WHERE window_start > now() - interval '7 days'
        GROUP BY torrent_id
        HAVING SUM(delta_up) - SUM(delta_down) > $1 AND COUNT(DISTINCT user_id) >= 2
        "#,
    )
    .bind(threshold)
    .fetch_all(db)
    .await?;

    let mut first_hits = 0u64;
    for (torrent_id, gap) in rows {
        // 免费促销豁免（与 hr_enforce 的免费判定同口径）。
        // 审计修复：官方范围（scope='official'）促销此前不看 kind 整条豁免——
        // 官方 x2（上传双倍、下载照计）也会把 up-down gap 洗成「免费」跳过审计。
        // 收窄为：仅下载侧免费类 kind（free/x2free）豁免（x2free 上传虽双倍，但下载为 0，
        // 天然产生 gap 且属官方促销口径，整条豁免）；其余 kind（x2/half/x2half/p30）一律不豁免。
        let exempt: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM promotions p \
             WHERE (p.torrent_id = $1 OR p.torrent_id IS NULL) \
               AND p.starts_at <= now() AND p.ends_at > now() \
               AND p.kind IN ('free','x2free'))",
        )
        .bind(torrent_id)
        .fetch_one(db)
        .await
        .unwrap_or(false);
        if exempt {
            continue;
        }
        let agent = format!("torrent:{torrent_id}");
        let existed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cheat_events WHERE \
             user_id = 0 AND agent = $1 AND reason = 'torrent_gap_audit')",
        )
        .bind(&agent)
        .fetch_one(db)
        .await
        .unwrap_or(true);
        sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, reason) VALUES (0, $1, 'torrent_gap_audit') \
             ON CONFLICT (user_id, agent, reason) DO UPDATE SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(&agent)
        .execute(db)
        .await?;
        if !existed {
            first_hits += 1;
            let body = format!(
                "种子 #{torrent_id} 近 7 天上传总量比下载总量高出约 {gap} 字节（阈值 {threshold_gb} GiB），\
                 存在虚报上传（假流量）嫌疑。明细见后台「作弊探测」，请人工核对做种者列表。"
            );
            let _: Result<_, _> = sqlx::query(
                "INSERT INTO staffmessages (user_id, subject, body, permission) \
                 SELECT MIN(id), '流量差额审计告警', $1, 'cheater' FROM users WHERE class_id >= 90",
            )
            .bind(body)
            .execute(db)
            .await;
        }
    }
    if first_hits > 0 {
        tracing::warn!(n = first_hits, "cheat_audit: new torrent-gap suspects");
        // 0228 运维 webhook：新嫌疑广播到 Discord/TG（尽力而为）
        webhook_broadcast(
            db,
            &format!(
                "🚨 cheat_audit 新增 {first_hits} 个流量差额嫌疑，详情见后台"
            ),
        )
        .await;
    }
    Ok(first_hits)
}

/// P1-8 Ratio Watch（GZ 口径柔性观察期）：
/// 分享率跌破阈值 → 一次性站内信警告 + 14 天观察期；期内恢复自动解除，到期仍跌破 → 停下载权并通知管理组。
/// 阈值/期限走 site_settings：ratio_watch_threshold（默认 0.4）、ratio_watch_days（默认 14）。
pub async fn ratio_watch(db: &PgPool) -> anyhow::Result<u64> {
    let threshold_f: f64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'ratio_watch_threshold'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<f64>().ok())
    .unwrap_or(0.4)
    .clamp(0.01, 10.0);
    let days: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'ratio_watch_days'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(14)
    .clamp(1, 90);

    // ① 新跌破者：进入观察期 + 一次性警告信
    let entered = sqlx::query(
        r#"
        WITH low AS (
            SELECT id, username, uploaded, downloaded FROM users
            WHERE status < 2 AND class_id < 90 AND downloaded > 0 AND ratio_watch_until IS NULL
              AND uploaded::float8 / downloaded::float8 < $1
        ),
        entered AS (
            UPDATE users u SET ratio_watch_until = now() + make_interval(days => $2::int), ratio_warned_at = now()
            FROM low WHERE u.id = low.id RETURNING low.id, low.username
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, id, '分享率警告（观察期）',
               '您好 ' || username || '，您的分享率已跌破站点警戒线。请在 ' || $2 || ' 天内恢复（做种/发布均可提升上传量），' ||
               '观察期结束仍未恢复将暂停下载权限。如有特殊情况请联系管理组。'
        FROM entered
        "#,
    )
    .bind(threshold_f)
    .bind(days)
    .execute(db)
    .await?
    .rows_affected();

    // ② 到期仍跌破：按 ratio_watch_action 分级处置（U5 §12.3：
    // warn=仅再警告 / limit_download=暂停下载+通知管理组，默认后者=现状 T3）
    let action: String = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'ratio_watch_action'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "limit_download".into());

    let (punished, notified) = if action == "warn" {
        let expired: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, username FROM users \
             WHERE status < 2 AND class_id < 90 AND download_enabled \
               AND ratio_watch_until IS NOT NULL AND ratio_watch_until < now() \
               AND downloaded > 0 AND uploaded::float8 / downloaded::float8 < $1",
        )
        .bind(threshold_f)
        .fetch_all(db)
        .await
        .unwrap_or_default();
        for (uid, uname) in &expired {
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 VALUES (NULL, $1, '分享率警告（观察期已满）', $2)",
            )
            .bind(uid)
            .bind(format!(
                "您好 {uname}，您的观察期已结束但分享率仍未恢复至 {threshold_f:.2}。本次仅提醒；持续未恢复可能影响下载权限。"
            ))
            .execute(db)
            .await;
        }
        (expired.len() as u64, 0u64)
    } else {
        let punished = sqlx::query(
            r#"
            WITH expired AS (
                UPDATE users SET download_enabled = FALSE
                WHERE status < 2 AND class_id < 90 AND download_enabled
                  AND ratio_watch_until IS NOT NULL AND ratio_watch_until < now()
                  AND downloaded > 0 AND uploaded::float8 / downloaded::float8 < $1
                RETURNING id, username
            )
            INSERT INTO staffmessages (user_id, subject, body, permission)
            SELECT id, 'Ratio Watch 到期处置', '用户 ' || username || '（#' || id || '）观察期结束仍未恢复分享率，已按规则暂停下载权限，请人工复核。', 'cheater'
            FROM expired
            "#,
        )
        .bind(threshold_f)
        .execute(db)
        .await?
        .rows_affected();
        (punished, punished)
    };
    let _ = notified;

    // ③ 期内恢复：自动解除观察
    let cleared = sqlx::query(
        "UPDATE users SET ratio_watch_until = NULL \
         WHERE ratio_watch_until IS NOT NULL \
           AND (downloaded = 0 OR uploaded::float8 / downloaded::float8 >= $1)",
    )
    .bind(threshold_f)
    .execute(db)
    .await?
    .rows_affected();

    if entered + punished + cleared > 0 {
        tracing::info!(entered, punished, cleared, "ratio_watch cycle");
    }
    Ok(entered + punished)
}

/// P2-11 登录 IP 跳变检测（/24 段近似）：24h 内单账号登录来源超过阈值个不同 /24 段 → 记录作弊探测。
/// 纯近似（无 GeoIP 依赖，教育网 DHCP 换段是常态），只记录供人工参考，不自动处置。
pub async fn multi_ip_check(db: &PgPool) -> anyhow::Result<u64> {
    let threshold: i64 = 8;
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT user_id, COUNT(DISTINCT split_part(host(ip), '.', 1) || '.' || split_part(host(ip), '.', 2) || '.' || split_part(host(ip), '.', 3)) AS segments
        FROM login_events
        WHERE created_at > now() - interval '24 hours' AND ip IS NOT NULL AND host(ip) LIKE '%.%'
        GROUP BY user_id
        HAVING COUNT(DISTINCT split_part(host(ip), '.', 1) || '.' || split_part(host(ip), '.', 2) || '.' || split_part(host(ip), '.', 3)) >= $1
        "#,
    )
    .bind(threshold)
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for (user_id, segments) in rows {
        sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, reason) VALUES ($1, 'login_ip_spread', 'multi_subnet_24h') \
             ON CONFLICT (user_id, agent, reason) DO UPDATE SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(user_id)
        .execute(db)
        .await?;
        let _ = segments;
        n += 1;
    }
    Ok(n)
}
