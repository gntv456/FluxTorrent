//! 登录/登出（M01）：POST /auth/login + /auth/logout。
//! 从 auth_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, Responder};

use crate::domain;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{client_ip, ip_banned, throttle};
use crate::state::AppState;

use super::login_origin::{is_form_body, origin_allowed};
use super::login_types::LoginReq;

/// 登录体既接受 JSON（本站前端）也接受 `application/x-www-form-urlencoded`
/// （0267）：NexusPHP 生态的登录习惯是表单 POST（`takelogin.php`
/// username/password），此前表单打进来会 400「Content type error」，
/// 于是 MoviePilot / autobrr 这类按 NP 习惯登录的工具一个都进不来。
#[post("/auth/login")]
pub async fn login(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Either<web::Json<LoginReq>, web::Form<LoginReq>>,
) -> DomainResult<impl Responder> {
    // 表单路径先过跨站来源闸（见 origin_allowed 注释）
    if is_form_body(&req) && !origin_allowed(&req) {
        return Err(DomainError::Forbidden);
    }
    let body = body.into_inner();
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
    let mut pow_token_used: Option<String> = None;
    {
        let lock_key = format!("acctlock:{}", body.username.to_lowercase());
        // P-67：旧写法 `get(...).unwrap_or(0)` = 「Redis 挂了就当没锁」，
        // 锁定判定形同虚设。改 fail-close：上方 throttle() 本就 fail-close，
        // Redis 不可用时登录根本到不了这里，不新增可用性风险。
        let fails: i64 = crate::http::counter_peek(&state, &lock_key).await?;
        if fails >= 5 {
            return Err(DomainError::Validation(
                "该账号因连续登录失败已被临时锁定，请 15 分钟后再试".into(),
            ));
        }
        // PoW 闸门（sha256-pow-v1）：fails >= 2（第 3 次尝试起）且未锁定时，
        // 要求 body 带有效 pow_token/pow_solution——每次尝试先付一段 CPU，
        // 把单账户定向爆破的成本从 0 抬到与合法用户同等算力。校验失败与
        // 密码错同点 counter_bump（token 经 GETDEL 一次性消费，防重放）。
        if (crate::gaps_http::POW_FAILS_FROM..5).contains(&fails) {
            let token = body.pow_token.clone().unwrap_or_default();
            let sol = body.pow_solution.clone().unwrap_or_default();
            if let Err(e) =
                crate::gaps_http::pow_verify(&state, &token, &sol).await
            {
                crate::http::counter_bump(&state, &lock_key, 900).await?;
                return Err(e);
            }
            pow_token_used = Some(token);
        }
    }
    // 登录限流（§5.7：按用户名 5 次/分钟防爆破同一账户 + IP 维度双防）。
    // P-76：IP 维度原来也是 5 次/分钟——**与用户名档同宽**，于是它挡不住
    // 的是「换用户名的字典爆破」（那本来由用户名档 + acctlock 负责），
    // 挡住的却是合法场景：家庭/宿舍/NAT/CGNAT 出口下 6 个成员在同一分钟
    // 各自登录，第 6 个开始全量 429（本仓闸门 AB 一次连跑四个探针号就撞死
    // 在这里，报「登录 6 次都拿不到 token」）。IP 档只该做「单出口别太吵」
    // 的粗闸，20 次/分钟够用（= 每秒 1/3 次，脚本爆破仍被用户名档 + 锁定
    // 两条线拦住）。
    throttle(&state, format!("login:{}", body.username)).await?;
    crate::http::throttle_limited(
        &state,
        format!("login-ip:{}", peer_ip),
        20,
    )
    .await?;
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
            let lock_key = format!("acctlock:{}", body.username.to_lowercase());
            // P-67：旧写法 `incr(...).unwrap_or(0)` 把「记不上失败数」当成
            // 「没超限」⇒ 永不锁定，改 fail-close（顺序不必调：Redis 挂时
            // 上方 throttle() 已经先拒，这段到不了）。
            crate::http::counter_bump(&state, &lock_key, 900).await?;
            // PoW 闸门区段内 token 已在入口消费；此处无额外记账
            crate::gaps_http::pow_noop();
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
    // 安全审计 P1-3（2026-10-06）：TOTP 失败同样递增账户锁定计数——旧版只在密码
    // 错误分支计数，密码已泄露者可对 6 位码低速持续爆破（换 IP 即绕 IP 维度限流）。
    if let Err(e) = crate::twofa_http::login_totp_check(
        &state,
        user.id,
        body.totp_code.unwrap_or(0),
    )
    .await
    {
        {
            let lock_key = format!("acctlock:{}", body.username.to_lowercase());
            // P-67：旧写法 `incr(...).unwrap_or(0)` 把「记不上失败数」当成
            // 「没超限」⇒ 永不锁定，改 fail-close（顺序不必调：Redis 挂时
            // 上方 throttle() 已经先拒，这段到不了）。
            crate::http::counter_bump(&state, &lock_key, 900).await?;
            // TOTP 失败同口径：PoW 记账在入口闸门已完成
            crate::gaps_http::pow_noop();
        }
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
    if let Some(_e) = super::email_verify::guard_login(&state, user.id).await {
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
            // P-103：文案指的路径必须真存在。登录页原先没有重发入口，
            // resend 表单只在 /email-verify 页——现在登录页挂上了
            // 「收不到激活邮件？」，文案按界面上的实际字面量写。
            "邮箱尚未激活：请到注册邮箱点击激活链接，\
             或在登录页点「收不到激活邮件？」进激活页重发"
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
        let lock_key = format!("acctlock:{}", body.username.to_lowercase());
        // 清不掉只记 warn：不能为清理动作把已成功的登录打成 5xx（键按 TTL 过期）。
        crate::http::counter_clear(&state, &lock_key).await;
        // PoW 挑战键一并清理（用过即删；未携带时是 no-op）
        if let Some(t) = &pow_token_used {
            crate::gaps_http::pow_clear(&state, t).await;
        }
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
