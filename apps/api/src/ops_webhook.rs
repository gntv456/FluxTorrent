//! 通知通道第三轨（0228，P2 触点 #22）：运维向 webhook 出口。
//!
//! 两路：Discord 频道 webhook（POST JSON）与 Telegram Bot sendMessage。
//! 只做「管理向重大事件」广播（新申请/作弊告警/DLQ），不做用户级订阅——
//! 用户级 TG 绑定走既有 tg_bind（0021）。发送尽力而为：失败落日志不阻断业务。

use crate::state::AppState;

async fn webhook_url(state: &AppState, name: &str) -> Option<String> {
    sqlx::query_scalar(
        "SELECT NULLIF(value, '') FROM site_settings WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
}

/// 广播一条运维通知（Discord + TG 双发，各自独立尽力而为）
pub async fn broadcast_ops(state: &AppState, text: &str) {
    let discord = webhook_url(state, "webhook_discord").await;
    let tg_token = webhook_url(state, "tg_bot_token").await;
    let tg_chat = webhook_url(state, "tg_chat_id").await;
    let client = reqwest::Client::new();
    if let Some(url) = discord {
        if let Err(e) = client
            .post(&url)
            .json(&serde_json::json!({ "content": text }))
            .send()
            .await
        {
            tracing::warn!(?e, "discord webhook send failed");
        }
    }
    if let (Some(token), Some(chat)) = (tg_token, tg_chat) {
        let url = format!("https://api.telegram.org/bot{token}/sendMessage");
        if let Err(e) = client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": chat,
                "text": text,
                "disable_web_page_preview": true,
            }))
            .send()
            .await
        {
            tracing::warn!(?e, "telegram send failed");
        }
    }
}

/// 广播包装（供 handler 侧 spawn 调用，不占请求时延）
pub fn broadcast_ops_spawn(state: &std::sync::Arc<AppState>, text: String) {
    let st = state.clone();
    actix_web::rt::spawn(async move {
        broadcast_ops(&st, &text).await;
    });
}
