//! Passkey 登录仪式（begin/finish）——从 passkey.rs 拆出（300 行门禁）。

use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use base64::Engine;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::elliptic_curve::sec1::FromEncodedPoint;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::passkey_util::{
    b64url, flags, rp_id, rp_id_hash, store_challenge, take_challenge,
};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

// ============ 登录（匿名）：begin / finish ============

#[derive(Deserialize)]
pub(super) struct BeginLoginBody {
    pub username: String,
}

#[post("/auth/passkey-login/begin")]
pub(super) async fn passkey_login_begin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BeginLoginBody>,
) -> DomainResult<impl Responder> {
    // 用户存在性与凭证列表（不泄敏感：匿名端点只回 cred_id 列表）
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(body.username.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let fake_challenge = if uid.is_none() {
        // 防枚举：不存在也回一个 challenge（必然验不过）
        let c = uuid::Uuid::new_v4().to_string();
        store_challenge(&state, format!("pklogin:{}", c), "0".into()).await;
        Some(c)
    } else {
        None
    };
    let uid = match uid {
        Some(u) => u,
        None => {
            let rp = rp_id(&state, &req).await;
            return Ok(ok(serde_json::json!({
                "challenge": fake_challenge.unwrap(),
                "rp_id": rp,
                "cred_ids": [],
            })));
        }
    };
    let creds: Vec<String> = sqlx::query_scalar(
        "SELECT cred_id FROM user_passkeys WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let challenge = uuid::Uuid::new_v4().to_string();
    store_challenge(&state, format!("pklogin:{}", challenge), uid.to_string())
        .await;
    Ok(ok(serde_json::json!({
        "challenge": challenge,
        "rp_id": rp_id(&state, &req).await,
        "cred_ids": creds,
    })))
}

#[derive(Deserialize)]
pub(super) struct FinishLoginBody {
    pub cred_id: String,
    pub client_data_json: String,
    pub auth_data: String,
    pub signature: String,
    pub sign_count: i64,
}

#[post("/auth/passkey-login/finish")]
pub(super) async fn passkey_login_finish(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FinishLoginBody>,
) -> DomainResult<HttpResponse> {
    let cd = serde_json::from_slice::<serde_json::Value>(&b64url(
        &body.client_data_json,
    ))
    .map_err(|_| DomainError::Validation("clientData 解析失败".into()))?;
    let challenge = cd
        .get("challenge")
        .and_then(|v| v.as_str())
        .ok_or(DomainError::Validation("缺少 challenge".into()))?;
    if cd.get("type").and_then(|v| v.as_str()) != Some("webauthn.get") {
        return Err(DomainError::Validation(
            "clientData 类型不符（期望 webauthn.get）".into(),
        ));
    }
    let uid_str = take_challenge(&state, format!("pklogin:{}", challenge))
        .await
        .ok_or(DomainError::Validation("登录会话已过期，请重新开始".into()))?;
    let uid: i64 = uid_str
        .parse()
        .map_err(|_| DomainError::Validation("验证失败".into()))?;
    let row: Option<(i64, Vec<u8>, i64, i64, i16, bool)> = sqlx::query_as(
        "SELECT u.id, pk.public_key, pk.sign_count, \
                COALESCE(u.class_id, 0), u.status, \
                u.must_reset_password \
             FROM user_passkeys pk JOIN users u ON u.id = pk.user_id \
             WHERE pk.cred_id = $1 AND pk.user_id = $2",
    )
    .bind(&body.cred_id)
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((user_id, pk, prev_count, class_id, status, must_reset)) = row
    else {
        return Err(DomainError::InvalidCredentials);
    };
    if status >= 2 {
        return Err(DomainError::Validation(
            "账号已被封禁（如误封可尝试自助解封或申诉）".into(),
        ));
    }
    // authenticatorData：RP ID hash + UV
    let auth_data = b64url(&body.auth_data);
    let expect_rp = Sha256::digest(rp_id(&state, &req).await.as_bytes());
    if rp_id_hash(&auth_data) != Some(expect_rp.into()) {
        return Err(DomainError::Validation("RP ID 不匹配".into()));
    }
    let Some(f) = flags(&auth_data) else {
        return Err(DomainError::Validation("authenticatorData 不完整".into()));
    };
    if f & 0x04 == 0 {
        return Err(DomainError::Validation(
            "该凭证未做用户在场验证（UV）".into(),
        ));
    }
    // sign_count 克隆检测（0 = 认证器不支持计数，跳过单调检查）
    if prev_count > 0 && body.sign_count <= prev_count {
        return Err(DomainError::Validation(
            "签名计数异常（疑似凭证被克隆），请重置 passkey".into(),
        ));
    }
    // 验签：ES256(P-256) over auth_data || SHA256(clientDataJSON)
    let point = p256::EncodedPoint::from_bytes(&pk)
        .map_err(|_| DomainError::Validation("凭证公钥损坏".into()))?;
    let vk = VerifyingKey::from_encoded_point(&point)
        .map_err(|_| DomainError::Validation("凭证公钥损坏".into()))?;
    let mut msg = auth_data.clone();
    msg.extend_from_slice(&Sha256::digest(b64url(&body.client_data_json)));
    let sig = Signature::from_slice(&b64url(&body.signature))
        .map_err(|_| DomainError::Validation("签名格式错误".into()))?;
    vk.verify(&msg, &sig)
        .map_err(|_| DomainError::InvalidCredentials)?;
    // 过账：计数 + last_used
    sqlx::query(
        "UPDATE user_passkeys SET sign_count = $2, last_used_at = now() \
         WHERE cred_id = $1",
    )
    .bind(&body.cred_id)
    .bind(body.sign_count)
    .execute(&state.repo.db)
    .await
    .ok();
    // 签发 JWT（与密码登录同 token 通道；passkey 是 possession 因素，
    // 不再叠加 TOTP；must_reset 用户仍走改密闸门）
    let ttl: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'jwt_ttl_hours')::bigint, 24)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(24)
    .clamp(1, 720);
    let token = state
        .jwt
        .issue(user_id, class_id as i32, ttl)
        .map_err(DomainError::Internal)?;
    state
        .repo
        .audit(Some(user_id), "login_passkey", Some(user_id))
        .await;
    Ok(ok(serde_json::json!({
        "token": token,
        "must_reset_password": must_reset,
        "user": { "id": user_id, "class_id": class_id },
    })))
}
