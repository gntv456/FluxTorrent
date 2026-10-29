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
        // P-437：邮箱一律按小写入库。找回口读的是 `WHERE email = $1`（入参
        // 已 trim+lowercase），写入口若保留大小写，注册时打成 `Zhang@Gmail.com`
        // 的人此后**永远**命中 0 行、收不到重置信，而响应刻意与成功一致
        // （防枚举）⇒ 静默不可恢复。归一化口径与 applications.rs:46 同形。
        email: body.email.trim().to_lowercase(),
        password: body.password.clone(),
    };
    domain::validate_register(&new_user)?;
    // 注册模式（U1 §11.5）：invite_only（默认）/ open / email_verify。
    // open 旁路邀请码（inviter 置 NULL，链路其余不变）；email_verify 建号后
    // 发激活信并由 guard_login 拦登录（0208 时该模式确为空壳，其注释与
    // 「已移除」的说法沿用至今已与实现不符——发信/拦截都在，设置中心也仍提供
    // 该选项，故这里改成对齐实现的说法）。
    let reg_mode: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'registration_mode'), 'invite_only')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "invite_only".into());
    // P-104 前置：选了「开放注册 + 邮件激活」却发不出信（SMTP 或站点基址未
    // 配）时，建号即死锁——email_verified_at 恒 NULL、登录被 guard_login 永久
    // 拦下、用户又收不到任何可用链接，只能等管理员手工激活。红线实测：旧写法
    // 在此配置下注册返回 200 并留下一个进不去的账号。现在如实拒绝、不建号。
    if reg_mode == "email_verify"
        && !crate::mailer::mail_deliverable(&state.repo.db).await
    {
        return Err(DomainError::MailUnconfigured);
    }
    // 注册自定义字段（0186）：required 校验前置（缺→拒绝，先于邀请码/验证码
    // 报出更明确的提示）；值形状校验同批，落库在建号后
    {
        // P-140（批三十九实测，P0）：这里必须与 `/register-fields` **同一口径**
        // 过滤模块。字段挂在某个模块键下时，模块关着 ⇒ 表单根本不展示它，
        // 而旧写法这里只看 `enabled AND show_on_register`、不查模块，
        // 于是「表单看不见、后端硬要」= 谁都注册不了（实测红线：
        // `400 「Label bc_off」为必填字段`，而 /register-fields 里没有这条）。
        let defs: Vec<(String, String, bool, String, serde_json::Value)> =
            sqlx::query_as(&format!(
                "SELECT key, label, required, type, options \
                 FROM user_field_defs \
                 WHERE enabled AND show_on_register AND {}",
                crate::modules::module_on_sql("")
            ))
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
                    .map_err(|e| {
                        DomainError::Validation(crate::fields::named(label, &e))
                    })?;
            }
        }
    }
    // P-686 预约窗口放行：reg_window 配了生效窗口（此刻在 open..close 内）
    // 时，即便 registration_mode 仍是 invite_only 也按开放注册放行——
    // 「未来时间自动开放注册」由窗口到点自然生效，站长无须守着切换。
    // 名额满/窗口外由 check_reg_gates 的窗口闸先行拒绝，不会走到这里。
    let window_active =
        super::register_gates::reg_window_active(&state.repo).await;
    let effective_open = reg_mode != "invite_only" || window_active;
    // 带码注册永远走邀请分支（核销 + 绑邀请人 + 不占开放轮名额），
    // 与窗口是否生效无关——开放期间邀请注册照常计邀请人的功。
    // P-689：窗口/同 IP 这两道「开放轮治理闸」只约束免邀请码入口。
    // 旧写法对带码请求也先跑 check_window / check_ip_limit，于是
    // 「名额已满…可通过邀请码注册」「可换用邀请码注册」这两句指引
    // 指向的出口本身也被同一道闸拦死（实测：窗口未开始/已结束/满额、
    // 同 IP 超限时带有效邀请码一律 400）。
    let using_invite = !body.invite_code.trim().is_empty();
    if !effective_open && !using_invite {
        return Err(DomainError::InviteInvalid);
    }
    // 验证码校验（防注册机）。0227 起按驱动分发：
    // - none（缺省）：自研算术题（captcha_id + captcha_answer）
    // - turnstile/recaptcha/hcaptcha：前端组件 token → 后端 siteverify
    let ip_early = client_ip(&req);
    let provider =
        crate::gaps_http::captcha_drivers::provider(&state.repo.db).await;
    // PoW 验证码（0372 M5）：captcha_provider=pow 时用自托管工作量证明
    // 替代算术题/第三方组件——注册链路的挑战用 scope=register 签发。
    if provider == "pow" {
        crate::gaps_http::pow_verify(
            &state,
            body.pow_token.trim(),
            body.pow_solution.trim(),
        )
        .await?;
    }
    let handled_by_driver = crate::gaps_http::captcha_drivers::verify(
        &state,
        &provider,
        &body.captcha_token,
        &ip_early,
    )
    .await?;
    if provider != "pow"
        && !handled_by_driver
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
    // 注册答题闸（0372 M5）：reg_quiz_enabled=yes 时须带一次性票据；
    // reg_quiz_pass_required=yes 时答错即拒（no 只记录不拦截）。
    let quiz_on =
        crate::repo::reg_quiz::read_setting(&state.repo, "reg_quiz_enabled")
            .await
            .map(|v| v == "yes")
            .unwrap_or(false);
    if quiz_on {
        if body.quiz_ticket.trim().is_empty() {
            return Err(DomainError::Validation(
                "请先完成注册答题后提交".into(),
            ));
        }
        let quiz_ip = client_ip(&req);
        let out = crate::gaps_http::quiz_verify(
            &state,
            &body.quiz_ticket,
            body.quiz_answer,
            &quiz_ip,
        )
        .await?;
        if !out.passed {
            let must = crate::repo::reg_quiz::read_setting(
                &state.repo,
                "reg_quiz_pass_required",
            )
            .await
            .map(|v| v == "yes")
            .unwrap_or(true);
            if must {
                return Err(DomainError::Validation(
                    "站规测验答错了，请重新抽题作答".into(),
                ));
            }
        }
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
    // 注册治理四道闸（0370 M4）：注册窗口 / 同 IP 限额 / 一次性邮箱；
    // 用户总量（maxusers）在 register_user 事务内校验 + advisory 锁防超发。
    // P-689：窗口名额与同 IP 额度也是 check-then-insert（事务外先读后写），
    // 突发并发会各自读到「还剩 1 个」一起建号（实测剩 1 个名额时 4 个
    // 并发全部成功）。直接注册把「闸检查 → 建号」放进同一把 PG 会话锁，
    // 邀请注册不占这两道额度、不取锁。
    let pass_hash = domain::hash_password(&new_user.password)?;
    // 单事务注册（repo 分层收编）：事务体在 repo/auth.rs register_user——
    // 建用户+消费邀请码+绑邀请人原子完成（审计修复口径不变），
    // 邀请码无效整体回滚并区分 InviteUsed/InviteInvalid。
    let user_repo = crate::repo::auth::AuthRepo {
        db: state.repo.db.clone(),
    };
    let gate = if using_invite {
        None
    } else {
        super::register_gates::DirectGateLock::acquire(&state.repo.db).await?
    };
    let result = async {
        super::register_gates::check_reg_gates(
            &state,
            &ip,
            &new_user.email,
            using_invite,
        )
        .await?;
        user_repo
            .register_user(
                &new_user,
                &pass_hash,
                body.invite_code.trim(),
                !body.invite_code.trim().is_empty(),
            )
            .await
    }
    .await;
    if let Some(lock) = gate {
        lock.release().await;
    }
    let (user_id, _inviter) = result?;
    // 同 IP 注册留痕（0370 M4）：只存 sha3-256 摘要，失败只告警不阻断
    super::register_gates::record_ip_signup(&state.repo, &ip, user_id).await;
    // 字段值落库（口径见 register_fields_store：三处同源、失败留痕不回滚）
    super::register_fields_store::store_field_values(
        &state,
        user_id,
        &body.fields,
    )
    .await;
    state
        .repo
        .audit(Some(user_id), "user_register", Some(user_id))
        .await;
    // 新手欢迎 PM（0424 P1-4 拆出 welcome_pm；P-477 已接通知偏好闸与
    // kind='system'）。这里只留调用，不再内联一份旧口径——内联块会绕开
    // 偏好闸，且与拆出的模块形成两份规则。
    super::welcome_pm::send_welcome_pm(&state.repo.db, user_id).await;
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
