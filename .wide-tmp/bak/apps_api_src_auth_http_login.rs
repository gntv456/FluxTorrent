//! 登录/登出（M01）：POST /auth/login + /auth/logout。
//! 从 auth_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};

use crate::domain;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{client_ip, ip_banned, throttle};
use crate::state::AppState;

use super::login_types::LoginReq;

#[post("/auth/login")]
pub async fn login(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LoginReq>,
) -> DomainResult<impl Responder> {
    // IP 封禁强制校验（ip_bans 此前仅管理面 CRUD，无请求入口拦截）
    let peer_ip = client_ip(&req);
    // UA（0096 风控证据）：区分「同一人多设备」与「凭据泄露换客户端」；截断防滥用
    let ua = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .chars()
        .take(300)
        .collect::<String>();
    if ip_banned(&state, &peer_ip).await {
        return Err(DomainError::Validation(
            "IP 已被封禁，请联系管理组".into(),
        ));
    }
    // 登录限流（§5.7：5 次/分钟/用户名 + 5 次/分钟/IP 双维度——
    // 原实现仅用户名维度，换用户名字典爆破同一账户不受限）
    throttle(&state, format!("login:{}", body.username)).await?;
    throttle(&state, format!("login-ip:{}", peer_ip)).await?;
    let user = state
        .repo
        .find_user_by_name(body.username.trim())
        .await?
        .ok_or(DomainError::InvalidCredentials);
    let user = match user {
        Ok(u) => u,
        Err(e) => {
            // 封禁账户的密码正确时：不给 403，而是落「密码正确但被封」事件后仍回
            // 失败口径 —— 防枚举与正常流一致（真实封禁提示走公开 /auth/ban-log）。
            // 申诉通道见下方 ban_appeal 分支（P0 修复：此前被封用户无任何自助申诉入口）。
            let _ = sqlx::query(
                "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES (NULL, NULLIF($1,'')::inet, false, $2, 4)",
            )
            .bind(&peer_ip)
            .bind(&ua)
            .execute(&state.repo.db)
            .await;
            return Err(e);
        }
    };
    if !domain::verify_password(&user.pass_hash, &body.password) {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 1)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        return Err(DomainError::InvalidCredentials);
    }
    // 2FA（启用者必须带 totp_code）。失败也落登录事件（reason=2：缺码/错码细分看返回错误）
    if let Err(e) = crate::twofa_http::login_totp_check(
        &state.repo.db,
        user.id,
        body.totp_code.unwrap_or(0),
    )
    .await
    {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 2)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        return Err(e);
    }
    // 0072 闲置停用拦截：dormant_mark（worker 每小时）给 90 天未登录且无做种的非员工账号打标。
    // 拦截放在密码/2FA 之后 —— 不给探测者区分「休眠账号是否存在」的信息差。
    if user.dormant_at.is_some() {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 3)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        return Err(DomainError::Validation(
            "账号因长期未登录已被停用，请通过『联系我们』附上用户名申请恢复"
                .into(),
        ));
    }
    let token = state
        .jwt
        .issue(user.id, user.class_id, 24)
        .map_err(DomainError::Internal)?;
    // 登录事件（控制面板账户概览 30 天活跃趋势；含 IP 供 ipcheck/maxlogin）
    let _ = sqlx::query(
        "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, true, $3, 0)",
    )
    .bind(user.id)
    .bind(&peer_ip)
    .bind(&ua)
    .execute(&state.repo.db)
    .await;
    // M28 插件 Hook：登录成功后分发
    state.plugins.dispatch_login(&state, user.id);
    // 安全加固（P1 XSS 面）：token 同时以 HttpOnly+SameSite=Lax cookie 下发。
    // 兼容期双轨：响应体仍带 token（存量前端 localStorage 口径），前端迁移完成后
    // 移除。cookie 必须根路径：Next RSC 服务端渲染页面时（/users/1 等）要读
    // flux_token 转发 Bearer 给 API，而页面请求路径不在 /api/v1 下——浏览器按
    // Path 属性不会携带该 cookie，服务端 cookies() 读不到 → 401 → notFound()。
    // HttpOnly 仍使 XSS 无法读取（require_auth 同时接受 Cookie，见 token_from_request）。
    let mut resp = ok(serde_json::json!({
        "token": token,
        "must_reset_password": user.must_reset_password,
        "user": { "id": user.id, "username": user.username, "class_id": user.class_id }
    }));
    let cookie = actix_web::cookie::Cookie::build("flux_token", token)
        .path("/")
        .max_age(actix_web::cookie::time::Duration::hours(24))
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
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
