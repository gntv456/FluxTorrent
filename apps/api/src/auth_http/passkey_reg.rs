//! Passkey 注册仪式（begin/finish）——从 passkey.rs 拆出（300 行门禁）。
//! 工具函数与 challenge 存取在 passkey.rs（super 可见）。

use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use base64::Engine;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::passkey_util::{
    b64url, rp_id, rp_id_hash, store_challenge, take_challenge,
};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 注册（登录态）：begin / finish ============

#[derive(Deserialize)]
pub(super) struct BeginRegBody {
    #[serde(default)]
    pub label: String,
}

#[post("/auth/passkeys/begin")]
pub(super) async fn passkey_begin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BeginRegBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let challenge = uuid::Uuid::new_v4().to_string();
    store_challenge(
        &state,
        format!("pkreg:{}", auth.id),
        format!("{}|{}", challenge, body.label.replace('|', " ")),
    )
    .await;
    Ok(ok(serde_json::json!({
        "challenge": challenge,
        "rp_id": rp_id(&state, &req).await,
        "user_id": auth.id,
    })))
}

#[derive(Deserialize)]
pub(super) struct FinishRegBody {
    pub cred_id: String,    // base64url
    pub public_key: String, // base64url 的 COSE/SEC1 公钥（前端从 attObj 提取）
    pub client_data_json: String,
    pub auth_data: String, // base64url
    pub sign_count: i64,
}

#[post("/auth/passkeys/finish")]
pub(super) async fn passkey_finish(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FinishRegBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // P-75：这是**登录态**端点，但一次会话被偷就走完它 = 永久后门，
    // 所以自己也上限量（10 次/分/人）。
    crate::http::rate_window(
        &state,
        &format!("pkreg:{}", auth.id),
        10,
        60,
    )
    .await?;
    let cred_id = body.cred_id.trim();
    if cred_id.is_empty() || cred_id.len() > 512 {
        return Err(DomainError::Validation("cred_id 长度不符".into()));
    }
    // P-75（P0）：cred_id 在库里是**全局唯一**（user_passkeys_cred_id_key），
    // 而下面的 ON CONFLICT 目标就是这个全局键、DO UPDATE 又不带 user_id
    // 条件 ⇒ 任何登录用户把 body 里的 cred_id 填成别人的凭证 ID
    // （匿名端点 passkey-login/begin 会按用户名回吐 cred_ids），
    // 就把**受害者的公钥换成自己的**；随后正常走登录仪式即可拿到受害者
    // 身份的 JWT —— 账号接管。服务端对 client 自报的
    // (cred_id, public_key, auth_data) 三元组不做一致性校验是根因，
    // 这里先把「越界覆盖」这条最直白的路堵死。
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM user_passkeys WHERE cred_id = $1",
    )
    .bind(cred_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(oid) = owner {
        if oid != auth.id {
            state
                .repo
                .audit(Some(auth.id), "passkey_takeover_attempt", None)
                .await;
            tracing::warn!(
                cred_id = %cred_id,
                owner = oid,
                attacker = auth.id,
                "passkey 绑定撞了别人的凭证 ID（已拒）"
            );
            return Err(DomainError::Forbidden);
        }
    }
    // 每人最多 5 把（PasskeysCard 口径）：会话被偷的人不该能一次埋几十把
    let held: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM user_passkeys WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if held >= 5 && owner.is_none() {
        return Err(DomainError::Validation(
            "每人最多绑定 5 把 passkey，请先解绑".into(),
        ));
    }
    let stored = take_challenge(&state, format!("pkreg:{}", auth.id))
        .await
        .ok_or(DomainError::Validation(
            "注册会话已过期，请重新开始绑定".into(),
        ))?;
    let (challenge, label) =
        stored.split_once('|').unwrap_or((stored.as_str(), ""));
    // clientData 校验：type 与 challenge
    let cd = serde_json::from_slice::<serde_json::Value>(&b64url(
        &body.client_data_json,
    ))
    .map_err(|_| DomainError::Validation("clientData 解析失败".into()))?;
    if cd.get("type").and_then(|v| v.as_str()) != Some("webauthn.create") {
        return Err(DomainError::Validation(
            "clientData 类型不符（期望 webauthn.create）".into(),
        ));
    }
    if cd.get("challenge").and_then(|v| v.as_str()) != Some(challenge) {
        return Err(DomainError::Validation("challenge 不匹配".into()));
    }
    // RP ID hash
    let auth_data = b64url(&body.auth_data);
    let expect_rp = Sha256::digest(rp_id(&state, &req).await.as_bytes());
    if rp_id_hash(&auth_data) != Some(expect_rp.into()) {
        return Err(DomainError::Validation(
            "RP ID 不匹配（前端 rpId 须与站点域名一致）".into(),
        ));
    }
    // 公钥（SEC1 uncompressed）：p256 VerifyingKey
    let pk_bytes = b64url(&body.public_key);
    let point = p256::EncodedPoint::from_bytes(&pk_bytes)
        .map_err(|_| DomainError::Validation("公钥格式错误".into()))?;
    // 只验「是合法公钥点」，不在此处构造验签器（登录侧再建）
    let _vk = p256::ecdsa::VerifyingKey::from_encoded_point(&point)
        .map_err(|_| DomainError::Validation("公钥解析失败".into()))?;
    sqlx::query(
        "INSERT INTO user_passkeys \
         (user_id, label, cred_id, public_key, sign_count) \
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (cred_id) \
         DO UPDATE SET public_key = $4, sign_count = $5 \
         WHERE user_passkeys.user_id = EXCLUDED.user_id",
    )
    .bind(auth.id)
    .bind(if label.is_empty() { "passkey" } else { label })
    .bind(cred_id)
    .bind(&pk_bytes)
    .bind(body.sign_count)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "passkey_register", None)
        .await;
    Ok(ok(serde_json::json!({ "bound": true })))
}
