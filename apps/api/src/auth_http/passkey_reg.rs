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
         DO UPDATE SET public_key = $4, sign_count = $5",
    )
    .bind(auth.id)
    .bind(if label.is_empty() { "passkey" } else { label })
    .bind(&body.cred_id)
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
