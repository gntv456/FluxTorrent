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
    // 账户级失败锁定（P2 安全小补）：连续失败 5 次锁 15 分钟（Redis 计数，
    // 成功登录即清零）。与既有 IP/用户名限流叠加——限流拦高频，锁定拦低慢
    // 定向爆破。锁定期内连正确密码也拒（不给探测反馈）。
    {
        use redis::AsyncCommands;
        let lock_key = format!("acctlock:{}", body.username.to_lowercase());
        let mut c = state.redis.clone();
        let fails: i64 = c.get(&lock_key).await.unwrap_or(0i64);
        if fails >= 5 {
            return Err(DomainError::Validation(
                "该账号因连续登录失败已被临时锁定，请 15 分钟后再试".into(),
            ));
        }
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
                "INSERT INTO login_events (user_id, ip, ok, \
                 user_agent, reason) VALUES (NULL, NULLIF($1,'')::inet, false, \
                 $2, 4)",
            )
            .bind(&peer_ip)
            .bind(&ua)
            .execute(&state.repo.db)
            .await;
            return Err(e);
        }
    };
    if !domain::verify_password(&user.pass_hash, &body.password) {
        // 账户级锁定计数 +1（15 分钟窗口累计；命中 5 次后见上方拦截）
        {
            use redis::AsyncCommands;
            let lock_key = format!("acctlock:{}", body.username.to_lowercase());
            let mut c = state.redis.clone();
            let _: i64 = c.incr(&lock_key, 1).await.unwrap_or(0);
            let _: () = c.expire(&lock_key, 900).await.unwrap_or(());
        }
        let _ = sqlx::query(
                        "INSERT INTO login_events (user_id, ip, ok, \
             user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 1)",
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
                        "INSERT INTO login_events (user_id, ip, ok, \
             user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 2)",
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
                        "INSERT INTO login_events (user_id, ip, ok, \
             user_agent, reason) VALUES ($1, NULLIF($2,'')::inet, false, $3, 3)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        // 0228 三档：归档档给出更明确文案（数据仍在，管理组可解档）
        if user.archived {
            return Err(DomainError::Validation(
                "账号因长期不活跃已被归档，请通过『联系我们』附上用户名申请解档"
                    .into(),
            ));
        }
        return Err(DomainError::Validation(
            "账号因长期未登录已被停用，请通过『联系我们』附上用户名申请恢复"
                .into(),
        ));
    }
    // C7-#4 邮箱激活拦截：email_verify 模式下未激活账号在此终止（密码/2FA
    // 之后，与 dormant 同位——不给探测者信息差）。resend 通道见 /auth/email/resend。
    if let Some(_e) =
        super::email_verify::guard_login(&state, user.id).await
    {
        let _ = sqlx::query(
            "INSERT INTO login_events (user_id, ip, ok, user_agent, reason) \
             VALUES ($1, NULLIF($2,'')::inet, false, $3, 5)",
        )
        .bind(user.id)
        .bind(&peer_ip)
        .bind(&ua)
        .execute(&state.repo.db)
        .await;
        return Err(DomainError::Validation(
            "邮箱尚未激活：请到注册邮箱点击激活链接，\
             或在登录页用注册邮箱重新发送激活邮件"
                .into(),
        ));
    }
    // 登录令牌有效期（0214 可配）：jwt_ttl_hours，缺省 24h = 既有口径；
    // cookie max-age 同步用同一值（令牌过期后 cookie 也应失效，避免带死票重试）
    let ttl_hours: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'jwt_ttl_hours')::bigint, 24)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(24)
    .clamp(1, 720);
    // 成功登录清锁定计数（P2 账户级锁定配套）
    {
        use redis::AsyncCommands;
        let lock_key = format!("acctlock:{}", body.username.to_lowercase());
        let mut c = state.redis.clone();
        let _: () = c.del(&lock_key).await.unwrap_or(());
    }
    let token = state
        .jwt
        .issue(user.id, user.class_id, ttl_hours)
        .map_err(DomainError::Internal)?;
    // 登录事件（控制面板账户概览 30 天活跃趋势；含 IP 供 ipcheck/maxlogin）
    let _ = sqlx::query(
        "INSERT INTO login_events (user_id, ip, ok, user_agent, \
         reason) VALUES ($1, NULLIF($2,'')::inet, true, $3, 0)",
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
        .max_age(actix_web::cookie::time::Duration::hours(ttl_hours))
        .http_only(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        // Secure flag（P2 安全小补）：请求已过 TLS（X-Forwarded-Proto=https 的
        // 反代场景）时显式加 secure——本地 http 开发不受影响。依赖站点按生产
        // 部署指南以 TLS 反代对外，此 flag 防 cookie 经明文 http 泄出。
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
