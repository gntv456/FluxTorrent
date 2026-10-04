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
    // 注册模式（U1 §11.5）：invite_only（默认，现状）/ open。
    // open 模式旁路邀请码（inviter 置 NULL，链路其余不变）。
    // （0208 P0：email_verify 选项已移除——旧实现不发验证信也不拦登录，
    // 留着是空壳；存量选了该值的站点由迁移归一为 open，行为本就等同。）
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
    // 验证码校验（防注册机）。0227 起按驱动分发：
    // - none（缺省）：自研算术题（captcha_id + captcha_answer）
    // - turnstile/recaptcha/hcaptcha：前端组件 token → 后端 siteverify
    let ip_early = client_ip(&req);
    let provider =
        crate::gaps_http::captcha_drivers::provider(&state.repo.db).await;
    let handled_by_driver = crate::gaps_http::captcha_drivers::verify(
        &state,
        &provider,
        &body.captcha_token,
        &ip_early,
    )
    .await?;
    if !handled_by_driver
        && (body.captcha_id.is_empty()
            || !crate::gaps_http::captcha_verify(
                &state,
                &body.captcha_id,
                body.captcha_answer,
            )
            .await)
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
    // 后缀策略（0282）：整类后缀准入门（先于个案封禁——后缀不过关时
    // 提示更聚焦「这类邮箱不能注册」）
    super::email_policy::check_email_policy(&state, &new_user.email).await?;
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
            if let Some(v) = body.fields.get(&key).filter(|v| !v.is_null()) {
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
    // 新手欢迎 PM（2026-10 链路测试修复）：新用户 0 魔力冷启动没有指引，
    // 第一局游戏前需要先签到。系统私信（sender_id NULL）讲清前三步；
    // 失败不回滚注册（附属数据，与字段值落库同口径）。
    {
        let site_name: String = sqlx::query_scalar(
            "SELECT value FROM site_settings WHERE name = 'site_name'",
        )
        .fetch_optional(&state.repo.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "站点".into());
        let body = format!(
            "欢迎！起步三步：\n1. 每天「任务中心 → 每日签到」领魔力与抽卡券；\
             \n2. 用魔力去「娱乐屋」玩一把（刮刮乐 10 魔力起）；\
             \n3. 抽卡的首次单抽只要 1 张券（票面 25 券），试试手气。\n\n\
             做种赚魔力才是正道——上传你的第一颗种子吧！"
        );
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body, \
             location, saved, unread) \
             VALUES (NULL, $1, $2, $3, 1, 1, true)",
        )
        .bind(user_id)
        .bind(format!("欢迎来到{site_name}！你的起步三步"))
        .bind(body)
        .execute(&state.repo.db)
        .await;
    }
    // C7-#4：email_verify 模式下注册即发激活信（失败不回滚账号，可 resend 补发）
    if reg_mode == "email_verify" {
        super::email_verify::send_verification(
            &state,
            user_id,
            &new_user.email,
            &new_user.username,
        )
        .await;
    }
    // E12 生命周期事件：新注册广播（用户名/第 N 位，不含邮箱等 PII）
    {
        let uname = new_user.username.clone();
        let total: i64 =
            sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(0);
        crate::ops_webhook::broadcast_ops_spawn(
            &state,
            format!("新用户注册：{uname}（第 {total} 位活跃成员）"),
        );
    }
    Ok(ok(serde_json::json!({ "user_id": user_id })))
}
