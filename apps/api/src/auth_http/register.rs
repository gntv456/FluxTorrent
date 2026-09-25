//! 注册（M01）：POST /auth/register。
//! 从 auth_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};

use crate::domain;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{client_ip, ip_banned, throttle};
use crate::state::AppState;

use super::login_types::RegisterReq;

#[post("/auth/register")]
pub async fn register(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RegisterReq>,
) -> DomainResult<impl Responder> {
    let new_user = domain::NewUser {
        username: body.username.trim().to_string(),
        email: body.email.trim().to_string(),
        password: body.password.clone(),
    };
    domain::validate_register(&new_user)?;
    // 注册模式（U1 §11.5）：invite_only（默认，现状）/ open / email_verify。
    // open 模式旁路邀请码（inviter 置 NULL，链路其余不变）；email_verify 走
    // 既有邮箱确认信（confirm/resend 已有），未确认账号登录受限由既有列控制。
    let reg_mode: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'registration_mode'), 'invite_only')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "invite_only".into());
    // 注册自定义字段（0186）：required 校验前置（缺→拒绝，先于邀请码/验证码
    // 报出更明确的提示）；值形状校验同批，落库在建号后
    {
        let defs: Vec<(String, String, bool, String, serde_json::Value)> =
            sqlx::query_as(
                "SELECT key, label, required, type, options FROM                  user_field_defs WHERE enabled AND show_on_register",
            )
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
        for (key, label, req, ftype, options) in &defs {
            let val = body.fields.get(key);
            if *req && (val.is_none() || val == Some(&serde_json::Value::Null))
            {
                return Err(DomainError::Validation(format!(
                    "「{label}」为必填字段"
                )));
            }
            if let Some(v) = val.filter(|v| !v.is_null()) {
                super::user_fields::validate_value_pub(ftype, options, v)
                    .map_err(DomainError::Validation)?;
            }
        }
    }
    if reg_mode == "invite_only" && body.invite_code.trim().is_empty() {
        return Err(DomainError::InviteInvalid);
    }
    // 图形验证码校验（防注册机）
    if body.captcha_id.is_empty()
        || !crate::gaps_http::captcha_verify(
            &state,
            &body.captcha_id,
            body.captcha_answer,
        )
        .await
    {
        return Err(DomainError::Validation("验证码错误或已过期".into()));
    }
    // 注册限流（§5.7）：按来源 IP 每分钟 5 次，防邀请码爆破
    let ip = client_ip(&req);
    // IP 封禁强制校验（与登录同口径；封禁名单由管理面维护）
    if ip_banned(&state, &ip).await {
        return Err(DomainError::Validation(
            "IP 已被封禁，请联系管理组".into(),
        ));
    }
    // 邮箱黑名单（0032 建表后首次接入注册链路）：pattern 三形态匹配——
    // 完整邮箱 / @domain（域名封禁）/ user@（前缀封禁）；allow 行优先豁免
    let email_banned: Option<bool> = sqlx::query_scalar(
        "SELECT NOT bool_or(mode = 'allow') FROM email_bans \
         WHERE lower($1) = lower(pattern) \
            OR (pattern LIKE '@%' AND lower($1) LIKE '%' || lower(pattern)) \
            OR (pattern LIKE '%@' AND lower($1) LIKE lower(pattern) || '%')",
    )
    .bind(&new_user.email)
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None)
    .flatten();
    if email_banned == Some(true) {
        return Err(DomainError::Validation(
            "该邮箱地址已被禁用，请联系管理组".into(),
        ));
    }
    throttle(&state, format!("register-ip:{ip}")).await?;
    let pass_hash = domain::hash_password(&new_user.password)?;
    // 单事务注册（repo 分层收编）：事务体在 repo/auth.rs register_user——
    // 建用户+消费邀请码+绑邀请人原子完成（审计修复口径不变），
    // 邀请码无效整体回滚并区分 InviteUsed/InviteInvalid。
    let user_repo = crate::repo::auth::AuthRepo {
        db: state.repo.db.clone(),
    };
    let (user_id, _inviter) = user_repo
        .register_user(
            &new_user,
            &pass_hash,
            body.invite_code.trim(),
            reg_mode == "invite_only",
        )
        .await?;
    // 字段值落库（required 已在前置校验；失败不回滚账号——附属数据）
    {
        let defs: Vec<String> = sqlx::query_scalar(
            "SELECT key FROM user_field_defs WHERE enabled AND              show_on_register",
        )
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
        for key in defs {
            if let Some(v) = body
                .fields
                .get(&key)
                .filter(|v| !v.is_null())
            {
                let _ = sqlx::query(
                    "INSERT INTO user_field_values (user_id, field_key,                      value) VALUES ($1, $2, $3) ON CONFLICT (user_id,                      field_key) DO NOTHING",
                )
                .bind(user_id)
                .bind(&key)
                .bind(v)
                .execute(&state.repo.db)
                .await;
            }
        }
    }
    state
        .repo
        .audit(Some(user_id), "user_register", Some(user_id))
        .await;
    Ok(ok(serde_json::json!({ "user_id": user_id })))
}
