//! 通知通道第三轨（0228，P2 触点 #22）：运维向 webhook 出口。
//!
//! 两路：Discord 频道 webhook（POST JSON）与 Telegram Bot sendMessage。
//! 只做「管理向重大事件」广播（新申请/作弊告警/DLQ），不做用户级订阅——
//! 用户级 TG 绑定走既有 tg_bind（0021）。
//!
//! 【0389 硬化】原实现是「拿到 URL 就 POST，失败记一行日志」，三处护栏全无：
//!   * **无 SSRF 防护**：URL 存在 site_settings 里，任何能改设置的人都能
//!     把它指向 `169.254.169.254`（云元数据）或内网端口，借 api 的身份
//!     扫内网。现在按 `domain::webhook::egress_allowed` 判（fail-closed）；
//!   * **无签名**：URL 常被贴进工单/截图/日志，收方无法验证来源，任何人
//!     都能伪造「作弊告警」钓鱼值班。现在带 HMAC-SHA256 签名 + 时间戳；
//!   * **无重试、无自动停用**：一次抖动就丢告警；持续失败则每个事件都
//!     重试，把连接池耗在一条死 URL 上。现在按配置重试，并达阈值自动停用。
//!
//! 【投递台账】每次投递落一行（出口/事件/结果/耗时）。此前「告警有没有
//! 送到」只存在于进程日志里，日志轮转即丢失。
//!
//! 【业务不被护栏拖累】三处失败路径都只记日志不返回错误：广播本来就是
//! 「尽力而为」，告警通道挂了不该让下架/作弊这些业务路径跟着失败。

use crate::repo::webhook as repo_wh;
use crate::state::AppState;

/// 单次投递的结果。
struct Delivery {
    attempts: i32,
    elapsed_ms: i32,
    ok: bool,
    detail: String,
}

/// 发一条到指定 URL。失败返回原因（不 panic、不阻断调用方）。
async fn post_signed(
    client: &reqwest::Client,
    url: &str,
    payload: &serde_json::Value,
    secret: &str,
    retries: i32,
) -> Delivery {
    let started = std::time::Instant::now();
    let body = payload.to_string();
    let ts = chrono::Utc::now().timestamp();

    let mut last = String::new();
    for attempt in 1..=retries.max(1) {
        let mut req =
            client.post(url).header("content-type", "application/json");
        if let Some(sig) = crate::domain::webhook::sign(secret, ts, &body) {
            req = req
                .header("X-Flux-Timestamp", ts.to_string())
                .header("X-Flux-Signature", sig);
        }
        match req.body(body.clone()).send().await {
            Ok(r) if r.status().is_success() => {
                return Delivery {
                    attempts: attempt,
                    elapsed_ms: started.elapsed().as_millis() as i32,
                    ok: true,
                    detail: format!("HTTP {}", r.status().as_u16()),
                };
            }
            Ok(r) => {
                last = format!("HTTP {}", r.status().as_u16());
            }
            Err(e) => {
                last = format!("{e}");
            }
        }
        if attempt < retries.max(1) {
            // 退避 500ms * 2^n：连续失败时立刻重试只会再失败一次。
            let back = 500u64 * 2u64.pow((attempt - 1) as u32);
            tokio::time::sleep(std::time::Duration::from_millis(
                back.min(8_000),
            ))
            .await;
        }
    }
    Delivery {
        attempts: retries.max(1),
        elapsed_ms: started.elapsed().as_millis() as i32,
        ok: false,
        // 失败原因截断：reqwest 的错误里可能带完整 URL（含 token）
        detail: last.chars().take(200).collect(),
    }
}

/// 记录并按阈值判断是否该停用。
async fn settle(
    state: &AppState,
    channel: &str,
    event: &str,
    d: &Delivery,
    threshold: i64,
) {
    let repo = &state.repo;
    let _ = repo_wh::record_delivery(
        repo,
        channel,
        event,
        if d.ok { "ok" } else { "failed" },
        d.attempts,
        d.elapsed_ms,
        &d.detail,
    )
    .await;
    if !d.ok && threshold > 0 {
        if let Ok(n) = repo_wh::consecutive_failures(repo, channel).await {
            if n >= threshold {
                tracing::error!(
                    channel, failures = n, threshold,
                    "webhook 连续失败达阈值，已自动停用该出口（改配置后可手工恢复）"
                );
            }
        }
    }
}

/// 广播一条运维通知（Discord + TG 双发，各自独立尽力而为）。
pub async fn broadcast_ops(state: &AppState, text: &str) {
    broadcast_ops_event(state, "ops", text).await;
}

/// 带事件标识的广播（投递台账按 event 归类）。
pub async fn broadcast_ops_event(state: &AppState, event: &str, text: &str) {
    let repo = &state.repo;
    let cidrs = repo_wh::egress_cidrs(repo).await.unwrap_or_default();
    let secret = repo_wh::setting(repo, "webhook_sign_secret")
        .await
        .unwrap_or_default();
    let retries: i32 = repo_wh::setting(repo, "webhook_retry_times")
        .await
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2)
        .clamp(0, 5);
    let threshold: i64 = repo_wh::setting(repo, "webhook_failure_threshold")
        .await
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
        .clamp(0, 1000);
    let timeout = std::time::Duration::from_secs(
        repo_wh::setting(repo, "webhook_timeout_secs")
            .await
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10)
            .clamp(1, 60),
    );

    // 连续失败达阈值的出口直接跳过：死 URL 上每条告警都重试，
    // 等于把连接池耗光。
    if repo_wh::is_disabled(repo, "discord", threshold)
        .await
        .unwrap_or(false)
    {
        tracing::warn!(channel = "discord", "出口已自动停用，跳过");
    } else {
        let url = repo_wh::setting(repo, "webhook_discord")
            .await
            .unwrap_or_default();
        // 未配置 = 这个出口不存在，不是「跳过整条广播」。
        // （早前版本在这里 return，会连带把 Telegram 也跳过——
        //   只配了 Telegram 的站点会从此收不到任何告警。）
        if !url.is_empty() {
            // redirect(Policy::none()) 不是可选项：出网判据只看**首跳**的
            // URL，禁了重定向才等于「只发到判过的那个地址」。
            // 否则一个白名单域名 302 到 169.254.169.254，整条 SSRF 护栏形同虚设。
            let client = reqwest::Client::builder()
                .timeout(timeout)
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_default();
            // SSRF 护栏：拦内网/云元数据/非 https。
            match crate::domain::webhook::egress_allowed(&url, &cidrs) {
                crate::domain::webhook::Egress::Deny(why) => {
                    tracing::error!(reason = %why, "webhook 出网被拒（discord）");
                    let _ = repo_wh::record_delivery(
                        repo, "discord", event, "failed", 0, 0, &why,
                    )
                    .await;
                }
                crate::domain::webhook::Egress::Allow => {
                    let d = post_signed(
                        &client,
                        &url,
                        &serde_json::json!({ "content": text }),
                        &secret,
                        retries,
                    )
                    .await;
                    if !d.ok {
                        tracing::warn!(
                            error = %d.detail, "discord webhook send failed"
                        );
                    }
                    settle(state, "discord", event, &d, threshold).await;
                }
            }
        }
    }

    // Telegram：URL 由 bot token 拼出，不受用户输入影响，但仍过一遍
    // 出网判据（fail-closed 的口径统一，免得以后有人改成可配置时忘了）。
    if repo_wh::is_disabled(repo, "telegram", threshold)
        .await
        .unwrap_or(false)
    {
        tracing::warn!(channel = "telegram", "出口已自动停用，跳过");
        return;
    }
    let tg_token = repo_wh::setting(repo, "tg_bot_token")
        .await
        .unwrap_or_default();
    let tg_chat = repo_wh::setting(repo, "tg_chat_id")
        .await
        .unwrap_or_default();
    if tg_token.is_empty() || tg_chat.is_empty() {
        return;
    }
    let url = format!("https://api.telegram.org/bot{tg_token}/sendMessage");
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap_or_default();
    match crate::domain::webhook::egress_allowed(&url, &cidrs) {
        crate::domain::webhook::Egress::Deny(why) => {
            tracing::error!(reason = %why, "webhook 出网被拒（telegram）");
        }
        crate::domain::webhook::Egress::Allow => {
            let d = post_signed(
                &client,
                &url,
                &serde_json::json!({
                    "chat_id": tg_chat,
                    "text": text,
                    "disable_web_page_preview": true,
                }),
                &secret,
                retries,
            )
            .await;
            if !d.ok {
                tracing::warn!(error = %d.detail, "telegram send failed");
            }
            settle(state, "telegram", event, &d, threshold).await;
        }
    }
}

/// 广播包装（供 handler 侧 spawn 调用，不占请求时延）。
pub fn broadcast_ops_spawn(state: &std::sync::Arc<AppState>, text: String) {
    let st = state.clone();
    actix_web::rt::spawn(async move {
        broadcast_ops(&st, &text).await;
    });
}

/// 带事件标识的广播包装。
pub fn broadcast_event_spawn(
    state: &std::sync::Arc<AppState>,
    event: &str,
    text: String,
) {
    let st = state.clone();
    let ev = event.to_string();
    actix_web::rt::spawn(async move {
        broadcast_ops_event(&st, &ev, &text).await;
    });
}
