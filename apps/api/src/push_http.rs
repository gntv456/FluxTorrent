//! M26 Web Push：订阅管理 + VAPID JWT + aes128gcm 载荷加密 + 投递。
//!
//! 实现遵循 RFC 8291（Message Encryption for WebPush, aes128gcm）与 RFC 8292（VAPID）。
//! 环境变量：`VAPID_PUBLIC_KEY` / `VAPID_PUBLIC_KEY`（P-256，未配置时推送端点禁用但订阅可管理）。

pub(crate) mod crypto;
pub(crate) mod endpoint;

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{rate_window, require_auth};
use crate::state::AppState;

use crypto::{decode_client_key, deliver, vapid_from_env, PushResult};
use endpoint::guard_endpoint;

pub fn mount_push(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(subscribe)
        .service(unsubscribe)
        .service(push_test)
        .service(vapid_public_key)
}

// ============ 订阅管理 ============

#[derive(Deserialize)]
struct SubscribeReq {
    endpoint: String,
    keys: Keys,
    #[serde(default = "default_topics")]
    topics: Vec<String>,
}
#[derive(Deserialize)]
struct Keys {
    p256dh: String,
    auth: String,
}
fn default_topics() -> Vec<String> {
    // P-370：必须含 "hr"。投递侧（push_notify.rs）发 H&R 预警/违规用的就是
    // topic="hr"，而 `push_to_user` 的过滤是 `$topic = ANY(p.topics)`——订阅端
    // 不收 "hr" 就等于全站 H&R 推送结构上永远发不出去（现网 push_outbox 里
    // 57 条 hr 事件全被标 processed_ok=true，活跃订阅 0 条）。
    vec![
        "promo".into(),
        "request".into(),
        "message".into(),
        "hr".into(),
    ]
}

/// 可订阅话题 = 投递侧 TOPICS 的同一张表（P-370 补 "hr"）。
/// 两侧词表必须一致：只改一侧就会「订阅得到、投递发不出」或反过来。
const VALID_TOPICS: &[&str] = &["promo", "request", "message", "hr"];

#[post("/push/subscribe")]
async fn subscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SubscribeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // P-454：这一口每次都要做一次 DNS 解析 + 一行 upsert，此前全站唯一的
    // 限流层是逐口挂的 `rate_window`，而 /push 三个口一个都没挂 ⇒ 登录态
    // 下可以无限刷（键名只传业务段，`rl:` 前缀由 throttle 统一加，P-72）。
    rate_window(&state, &format!("pushsub:{}", auth.id), 30, 60).await?;
    if !body.endpoint.starts_with("https://") {
        return Err(DomainError::Validation(
            "endpoint 必须为 https 推送服务地址".into(),
        ));
    }
    // P-453（取代 P2 的字符串前缀白名单）：endpoint 由会员自带，站点会向它
    // POST（E2E 加密载荷），所以这是**出网口**。旧写法只比字符串前缀，
    // `https://169-254-169-254.nip.io/`（公共通配 DNS，A 记录就是云元数据）
    // 与自建域指回 127.0.0.1 都能通过；且 `if let Ok(..)` 让「URL 解析失败」
    // 直接跳过检查（解析不了＝放行）。现在按解析后的**每一个 IP** 判公网，
    // 解析不出来一律拒。
    if guard_endpoint(&body.endpoint).await.is_none() {
        return Err(DomainError::Validation(
            "endpoint 必须是可解析到公网地址的 https 推送服务地址".into(),
        ));
    }
    for t in &body.topics {
        if !VALID_TOPICS.contains(&t.as_str()) {
            return Err(DomainError::Validation(format!(
                "topic 仅支持 {VALID_TOPICS:?}"
            )));
        }
    }
    // 公钥可解析性前置校验（坏 base64/非 P-256 提前报参数错而不是投递时才炸）
    decode_client_key(&body.keys.p256dh)
        .map_err(|e| DomainError::Validation(format!("p256dh 无效: {e}")))?;

    sqlx::query(
        "INSERT INTO push_subscriptions (user_id, endpoint, p256dh, auth, topics) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (endpoint) DO UPDATE SET \
            user_id = EXCLUDED.user_id, p256dh = EXCLUDED.p256dh, \
            auth = EXCLUDED.auth, topics = EXCLUDED.topics, expired_at = NULL",
    )
    .bind(auth.id)
    .bind(&body.endpoint)
    .bind(&body.keys.p256dh)
    .bind(&body.keys.auth)
    .bind(&body.topics)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "subscribed": body.endpoint, "topics": body.topics }),
    ))
}

#[derive(Deserialize)]
struct UnsubscribeReq {
    endpoint: String,
}

#[post("/push/unsubscribe")]
async fn unsubscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<UnsubscribeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 退订与订阅同档：软删一行很便宜，但两口成对刷（订阅→退订→订阅）
    // 会让 upsert 的写放大翻倍，同档才不用解释为什么一个是 30 一个是 300。
    rate_window(&state, &format!("pushunsub:{}", auth.id), 30, 60).await?;
    let n = sqlx::query(
        "UPDATE push_subscriptions SET expired_at = now() \
         WHERE endpoint = $1 AND user_id = $2 AND expired_at IS NULL",
    )
    .bind(&body.endpoint)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("订阅不存在或已退订".into()));
    }
    Ok(ok(serde_json::json!({ "unsubscribed": body.endpoint })))
}

#[derive(sqlx::FromRow)]
struct SubRow {
    id: i64,
    endpoint: String,
    p256dh: String,
    auth: String,
}

/// 测试推送（给自己发一条）—— 验证订阅-投递-退订闭环的入口
#[post("/push/test")]
async fn push_test(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 测试推送是**唯一会真向外发 HTTP POST** 的口（每次最多 5 条订阅），
    // 所以按「签发 Token」那一档的 5/分收，而不是读取口的 60/时。
    rate_window(&state, &format!("pushtest:{}", auth.id), 5, 60).await?;
    let subs: Vec<SubRow> = sqlx::query_as(
        "SELECT id, user_id, endpoint, p256dh, auth FROM push_subscriptions \
         WHERE user_id = $1 AND expired_at IS NULL LIMIT 5",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if subs.is_empty() {
        return Err(DomainError::Validation(
            "尚未订阅推送（浏览器开启通知后重试）".into(),
        ));
    }
    let vapid = vapid_from_env();
    let mut delivered = 0usize;
    let mut gone = 0usize;
    for s in &subs {
        match deliver(
            &s.endpoint,
            &s.p256dh,
            &s.auth,
            "FluxTorrent",
            "推送链路测试 ✅",
            &vapid,
        )
        .await
        {
            PushResult::Delivered | PushResult::ServiceError => delivered += 1,
            PushResult::Gone => {
                gone += 1;
                let _ = sqlx::query(
                    "UPDATE push_subscriptions \
                     SET expired_at = now() WHERE id = $1",
                )
                .bind(s.id)
                .execute(&state.repo.db)
                .await;
            }
        }
    }
    Ok(ok(
        serde_json::json!({ "delivered": delivered, "expired_cleaned": gone }),
    ))
}

#[get("/push/vapid-key")]
async fn vapid_public_key() -> impl Responder {
    let pub_b64 = std::env::var("VAPID_PUBLIC_KEY").unwrap_or_default();
    ok(serde_json::json!({
        "public_key": pub_b64,
        "enabled": !pub_b64.is_empty(),
    }))
}
