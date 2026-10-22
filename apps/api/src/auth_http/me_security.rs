//! me 安全面（M01）：passkey 轮换 + 改密码。
//! 从 auth_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::domain;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{bump_guard_ver, require_auth};
use crate::state::AppState;

#[post("/me/passkey/rotate")]
pub async fn rotate_passkey(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PasskeyRotateReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // P-409：一次轮换同时发新钥、废旧钥、撤下载键与 RSS token，且**不触发
    // 任何登出联动**——会话被偷的人不只白拿一份新的 tracker/RSS 身份，还能
    // 把原主所有下载器与命名订阅打成 401。与改密同一把尺：先复核本人。
    // 复核要试密码，故与 pwchange 同样按用户限流，别把这里变成新的 oracle。
    crate::http::rate_window(&state, &format!("pkrotate:{}", auth.id), 5, 60)
        .await?;
    crate::twofa_http::step_up(
        &state,
        auth.id,
        &body.password,
        body.code,
        "重置密钥",
    )
    .await?;
    // 自助「重置密钥」= 安全处置动作 ⇒ grace=false，旧钥当场失效
    // （留宽限窗等于没撤销；改密顺带轮换那条路才给宽限，见
    // repo::update_passkey 与 me_security 的改密分支）
    let pk = state.repo.update_passkey(auth.id, false).await?;
    // 轮换联动（0352/竞品对标批）：旧钥名下的一切凭据随处置动作一并作废——
    // 未过期下载键（30 分钟窗内可复用）与 RSS token（fxr_，与 passkey 解耦
    // 的命名订阅凭证）都不该活过一次「我怀疑密钥泄露」的轮换。
    sqlx::query(
        "DELETE FROM download_keys WHERE user_id = $1 \
         AND expires_at > now()",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE rss_tokens SET revoked_at = now() \
         WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "passkey_rotate", Some(auth.id))
        .await;
    // 旧 passkey 随 guard 版本轮询失效（tracker 侧 ≤3s，见 apps/tracker/src/main.rs
    // 的 flux:guard:ver 轮询清缓存）——不是字面「立即」，文案与注释同口径。
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "passkey": pk })))
}

/// 自助修改密码（usercp 口径）：验旧密码 → 换哈希 → 撤销既有 token，
/// 并给本设备重签（其它设备全部下线，本设备凭新 token 继续使用）。

#[post("/me/password/change")]
pub async fn me_password_change(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PasswordChangeReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.new_password.len() < 8 {
        return Err(DomainError::Validation("新密码至少 8 位".into()));
    }
    if body.new_password == body.old_password {
        return Err(DomainError::Validation("新密码不能与旧密码相同".into()));
    }
    // P-498：这里是登录之外的第二个密码 oracle——登录侧有 acctlock（失败
    // 计数 + PoW），这一口原先不限次：会话被偷即可无限猜旧密码，猜中后
    // 顺手改密，把原主踢下线（改密会撤销全部旧 token）并永久夺号。
    // 现网实测 12 次错旧密码全 400、无 429。按用户与登录同档限流。
    crate::http::rate_window(&state, &format!("pwchange:{}", auth.id), 5, 60)
        .await?;
    let (pass_hash,): (String,) =
        sqlx::query_as("SELECT pass_hash FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if !domain::verify_password(&pass_hash, &body.old_password) {
        state
            .repo
            .audit(Some(auth.id), "password_change_fail", Some(auth.id))
            .await;
        return Err(DomainError::Validation("旧密码不正确".into()));
    }
    let new_hash = domain::hash_password(&body.new_password)?;
    // 改密同时轮换 passkey：引导期 root 等公开默认 passkey 不应在改密后继续可用。
    // 旧钥进 passkey_prev 宽限窗（审计 10-07 P1-4）：passkey 烤在用户已下载的
    // 每一个 .torrent 里，即刻失效等于手上所有种子集体停种。
    let new_passkey = uuid::Uuid::new_v4().simple().to_string();
    let rotated: Option<String> = sqlx::query_scalar(
        "UPDATE users SET pass_hash = $2, passkey = $3, \
            must_reset_password = false, \
            passkey_prev = passkey, \
            passkey_prev_until = now() + ($4::bigint * interval '1 hour') \
         WHERE id = $1 RETURNING passkey",
    )
    .bind(auth.id)
    .bind(&new_hash)
    .bind(&new_passkey)
    .bind(domain::passkey_grace_hours())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 改密必须同时失效 5s 状态缓存：must_reset_password 是这条缓存的字段之一，
    // 不失效的话，用户改完密（尤其装机时 root 首次强制改密）最多 5 秒内仍被判
    // 「账号正在使用临时密码」，向导下一步 POST /setup 直接吃 400。
    state.user_status_cache.invalidate(auth.id);
    // P-563：改密也要让此前寄出的找回链接失效——攻击者手里待用的那枚
    // 不能把刚换掉的密码再改回去（与重置口的「一次性」同一把尺）。
    sqlx::query(
        "UPDATE password_resets SET used_at = now() \
         WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = rotated;
    state
        .repo
        .audit(
            Some(auth.id),
            "passkey_rotate_on_password_change",
            Some(auth.id),
        )
        .await;
    // P-565：撤销线要覆盖「改密前任何时刻签发的凭证」。旧口径 nbf=本
    // token 的 iat−1 只判死更早的会话——改密前在别处登录（iat 更晚）的
    // 会话反而幸存，与「其它设备全部失效」的文案不符。
    // nbf=now−1 让所有既有 token 失效，同时给本设备重签一枚 iat=now 的
    // 新 token 随响应写回 cookie（本设备不受影响）；与本次改密同一秒内
    // 签发的其它 token 也会幸存——与踢出会话口同一处可接受边界。
    let nbf = chrono::Utc::now().timestamp() - 1;
    let ttl_hours: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'jwt_ttl_hours')::bigint, 24)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(24)
    .clamp(1, 720);
    let token = state
        .jwt
        .issue(auth.id, auth.class_id, ttl_hours)
        .map_err(DomainError::Internal)?;
    let _ = sqlx::query(
        "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE SET nbf = \
         GREATEST(token_revocations.nbf, EXCLUDED.nbf), updated_at = now()",
    )
    .bind(auth.id)
    .bind(nbf)
    .execute(&state.repo.db)
    .await;
    let mut c = state.redis.clone();
    let _: () = redis::AsyncCommands::set_ex(
        &mut c,
        format!("logout_nbf:{}", auth.id),
        nbf,
        86400u64,
    )
    .await
    .unwrap_or(());
    state
        .repo
        .audit(Some(auth.id), "password_change", Some(auth.id))
        .await;
    let mut resp = ok(serde_json::json!({
        "changed": true,
        "token": token.clone()
    }));
    let cookie = actix_web::cookie::Cookie::build("flux_token", token)
        .path("/")
        .max_age(actix_web::cookie::time::Duration::hours(ttl_hours))
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .secure(
            req.headers()
                .get("x-forwarded-proto")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.eq_ignore_ascii_case("https"))
                .unwrap_or(false),
        )
        .finish()
        .to_string();
    use actix_web::http::header::{HeaderName, HeaderValue};
    resp.headers_mut().insert(
        HeaderName::from_static("set-cookie"),
        HeaderValue::from_str(&cookie)
            .expect("cookie 串解析为 HeaderValue 必然成功"),
    );
    Ok(resp)
}

#[derive(Deserialize)]
pub(super) struct PasswordChangeReq {
    pub(super) old_password: String,
    pub(super) new_password: String,
}

/// P-409：`{}`（旧前端/裸按钮）也走 step_up 的中文「需先复核密码」，
/// 不让 actix 的解析错误先把它折成一句无上下文的坏请求。
#[derive(Deserialize)]
pub(super) struct PasskeyRotateReq {
    #[serde(default)]
    pub(super) password: String,
    #[serde(default)]
    pub(super) code: Option<u32>,
}
