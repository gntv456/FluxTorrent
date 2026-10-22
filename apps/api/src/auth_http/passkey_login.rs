//! Passkey 登录仪式（begin/finish）——从 passkey.rs 拆出（300 行门禁）。

use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use base64::Engine;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::elliptic_curve::sec1::FromEncodedPoint;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::passkey_util::{
    b64url, flags, origin_allowed, rp_id, rp_id_hash, store_challenge,
    take_challenge,
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
    // P-79：匿名端点，且每次都要打一次库 + 一次 Redis 写。密码登录那侧
    // 有 5 次/分（按用户名）+ IP 档 + 账号锁定，这里原本一条限流都没有
    // ——既是用户名枚举的放大器，也是免费的算力放大口。
    let ip = crate::http::client_ip(&req);
    crate::http::rate_window(&state, &format!("pkloginb:{ip}"), 20, 60).await?;
    crate::http::rate_window(
        &state,
        &format!("pkloginb-u:{}", body.username.trim().to_lowercase()),
        10,
        60,
    )
    .await?;
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
            let rp = rp_id(&req);
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
    // P-80：`unwrap_or_default()` 把「查崩了」写成「你没有 passkey」——
    // 一次 DB 抖动就让正常用户在自己的凭证页上看到空列表（并被引导去
    // 重复绑定）。读侧的错要如实报出去。
    .map_err(|e| DomainError::Internal(e.into()))?;
    let challenge = uuid::Uuid::new_v4().to_string();
    store_challenge(&state, format!("pklogin:{}", challenge), uid.to_string())
        .await;
    Ok(ok(serde_json::json!({
        "challenge": challenge,
        "rp_id": rp_id(&req),
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
    // P-79：认证端点 + 每次一把 ECDSA 验签，此前无任何上限。
    let ip = crate::http::client_ip(&req);
    crate::http::rate_window(&state, &format!("pkloginf:{ip}"), 10, 60).await?;
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
    // P-566：rpId 只圈到注册域，同域兄弟子域也能拿到合法签名——断言来源
    // 必须钉到自家页面。校验放在取 challenge 之前：来源不对不烧掉会话。
    let origin = cd
        .get("origin")
        .and_then(|v| v.as_str())
        .ok_or(DomainError::Validation("clientData 缺少 origin".into()))?;
    if !origin_allowed(&req, origin) {
        return Err(DomainError::Validation(
            "来源不符：passkey 只接受本站页面的断言".into(),
        ));
    }
    let uid_str = take_challenge(&state, format!("pklogin:{}", challenge))
        .await
        .ok_or(DomainError::Validation("登录会话已过期，请重新开始".into()))?;
    let uid: i64 = uid_str
        .parse()
        .map_err(|_| DomainError::Validation("验证失败".into()))?;
    // P-78：第三列 `COALESCE(u.class_id, 0)` 是 **integer(INT4)**，按 i64 解
    // 必然 `mismatched types; Rust type i64 (as SQL type INT8) is not
    // compatible with SQL type INT4` ⇒ 这条查询从写下起就没成功过一次，
    // passkey 登录**全量 500**（注册侧却是 200，于是界面上「绑定成功」，
    // 谁都用不了）。类型要和列本身对齐，不是靠 `as i32` 蒙过去。
    let row: Option<(i64, Vec<u8>, i64, i32, i16, bool)> = sqlx::query_as(
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
    let expect_rp = Sha256::digest(rp_id(&req).as_bytes());
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
    // P-81：CTAP2/WebAuthn 规定 `sig` 是 **ASN.1 DER**（ECDSA-sig-structure），
    // 而 `Signature::from_slice` 只吃定宽 64 字节 r‖s（ecdsa-0.16.9
    // /src/lib.rs:224-230），DER 得走 `from_der`（同文件 232-236 行，
    // `der` feature 已由 p256 打开，cargo tree 可查）。用 from_slice 的实际
    // 后果：任何浏览器/认证器交回来的签名都在这里被判「签名格式错误」——
    // 也就是说 passkey 登录对**真实客户端从来没通过过**。实测同一份签名的
    // 两种编码：DER ⇒ 400 签名格式错误；把它转成 r‖s ⇒ 200 + 本人 JWT。
    let sig = Signature::from_der(&b64url(&body.signature))
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
        .issue(user_id, class_id, ttl)
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
