//! 运维 webhook 广播（0501 从 audit.rs 拆出——行数门禁）。
//! 超时收口历史见函数注释。

use sqlx::PgPool;

/// 0228 运维 webhook 广播（worker 侧：直接查 site_settings + reqwest，
/// 失败只落日志）。
/// 0501 超时收口：「webhook 超时存量坏」记忆条目的真身——reqwest
/// 默认无总超时，Discord/TG 黑洞时调用方（cheat_audit/hr 等告警链）
/// 陪着挂。对齐 api 侧 ops_webhook 的 webhook_timeout_secs 键（缺省
/// 10s，clamp 1-60），双通道各限一次、不再无界等待。
pub(crate) async fn webhook_broadcast(db: &PgPool, text: &str) {
    let keys: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN \
         ('webhook_discord', 'tg_bot_token', 'tg_chat_id', \
          'webhook_timeout_secs')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let kv: std::collections::HashMap<_, _> = keys.into_iter().collect();
    let timeout_secs: u64 = kv
        .get("webhook_timeout_secs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
        .clamp(1, 60);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    if let Some(url) = kv.get("webhook_discord").filter(|u| !u.is_empty()) {
        let body = serde_json::json!({ "content": text });
        let _ = client.post(url).json(&body).send().await;
    }
    let tg = (
        kv.get("tg_bot_token").filter(|t| !t.is_empty()),
        kv.get("tg_chat_id").filter(|c| !c.is_empty()),
    );
    if let (Some(token), Some(chat)) = tg {
        let url = format!("https://api.telegram.org/bot{token}/sendMessage");
        let _ = client
            .post(url)
            .json(&serde_json::json!({ "chat_id": chat, "text": text }))
            .send()
            .await;
    }
}
