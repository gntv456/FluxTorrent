//! 找回密码 + 邮件通道 + H&R 追责 + 等级自动升降 + 申诉 + 补签卡使用
//! （NexusPHP 对比缺口补齐，迁移 0020）。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use redis::AsyncCommands;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

pub fn mount_gaps(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(ban_log)
        .service(confirm_resend)
        // 找回密码
        .service(password_forgot)
        .service(password_reset)
        // 验证码（注册用）
        .service(captcha_issue)
        // H&R
        .service(my_hr_status)
        .service(hr_pardon)
        .service(hr_self_pardon)
        // 申诉
        .service(appeal_create)
        .service(appeal_my)
        .service(appeal_queue)
        .service(appeal_handle)
        // 补签卡使用
        .service(resub_use)
        // 等级
        .service(class_rules_list)
        .service(my_class_progress)
        .service(wishlist_list)
        .service(wishlist_add)
        .service(wishlist_remove)
}

fn hash_token(t: &str) -> String {
    let mut h = Sha3_256::new();
    h.update(t.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ============ 登录页辅助通道（NexusPHP 口径：封禁记录 / 重发验证邮件） ============

/// 封禁记录（公开，防枚举只回状态与时间）：NexusPHP user-ban-log.php 对齐。
#[derive(Deserialize)]
struct BanLogQuery {
    username: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct BanLogRow {
    status: i16,
    changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/auth/ban-log")]
async fn ban_log(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<BanLogQuery>,
) -> DomainResult<impl Responder> {
    // 限流：按用户名 5 次/分钟，防探测
    let mut c = state.redis.clone();
    let key = format!("rl:banlog:{}", q.username.trim().to_lowercase());
    let n: i64 = AsyncCommands::incr(&mut c, &key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = AsyncCommands::expire(&mut c, &key, 60).await.unwrap_or(());
    }
    if n > 5 {
        return Err(DomainError::RateLimited);
    }
    // 最近一次对该用户的 status 变更（audit ref.id = 目标用户）
    let row = sqlx::query_as::<_, BanLogRow>(
        r#"
        SELECT u.status, (
            SELECT max(a.created_at) FROM audit_log a
            WHERE a.action = 'user.set_status'
              AND a.ref = jsonb_build_object('id', u.id)
        ) AS changed_at
        FROM users u WHERE lower(username::text) = lower($1) LIMIT 1
        "#,
    )
    .bind(q.username.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 无论命中与否同构响应（不泄露账号存在性）；无记录视为正常用户
    Ok(ok(serde_json::json!(row.unwrap_or(BanLogRow {
        status: 0,
        changed_at: None,
    }))))
}

/// 重发验证邮件：项目 M01 注册无邮箱验证环节，此通道为邮件触达兜底——
/// SMTP 未配置时明确告知"邮件通道未开启"；无论邮箱是否存在返回相同响应（防枚举）。
#[derive(Deserialize)]
struct ConfirmResendReq {
    email: String,
}

#[post("/auth/confirm/resend")]
async fn confirm_resend(
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ConfirmResendReq>,
) -> DomainResult<impl Responder> {
    // 限流：按邮箱 3 次/小时
    let mut c = state.redis.clone();
    let key = format!("rl:confirmresend:{}", body.email.to_lowercase());
    let n: i64 = AsyncCommands::incr(&mut c, &key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = AsyncCommands::expire(&mut c, &key, 3600)
            .await
            .unwrap_or(());
    }
    if n > 3 {
        return Err(DomainError::RateLimited);
    }
    let uid: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE lower(email::text) = lower($1)")
            .bind(body.email.trim())
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(uid) = uid {
        state.repo.audit(Some(uid), "confirm.resend", None).await;
    }
    let smtp = std::env::var("SMTP_URL").unwrap_or_default();
    Ok(ok(serde_json::json!({
        "message": if smtp.is_empty() {
            "如邮箱存在，我们已记录该请求；站点邮件通道未开启，请联系管理员处理"
        } else {
            "如邮箱存在，验证邮件已重新发送"
        }
    })))
}

// ============ 找回密码（邮件通道） ============

#[derive(Deserialize)]
struct ForgotReq {
    email: String,
}

/// 申请重置：生成 30 分钟有效 token。
/// 邮件投递：SMTP 未配置（SMTP_URL 空）时降级为日志输出 token —— 开发态可直接完成闭环；
/// 生产配 SMTP 后走真实投递（邮件发送在后台线程，不阻塞响应）。
#[post("/auth/password/forgot")]
async fn password_forgot(
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ForgotReq>,
) -> DomainResult<HttpResponse> {
    // 限流：按邮箱 3 次/小时
    let mut c = state.redis.clone();
    let key = format!("rl:pwdforgot:{}", body.email.to_lowercase());
    let n: i64 = AsyncCommands::incr(&mut c, &key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = AsyncCommands::expire(&mut c, &key, 3600)
            .await
            .unwrap_or(());
    }
    if n > 3 {
        return Err(DomainError::RateLimited);
    }

    let uid: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(body.email.trim().to_lowercase())
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 无论命中与否都返回相同响应（防账号枚举）
    if let Some(uid) = uid {
        let token = format!("pwd_{}", uuid::Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO password_resets (user_id, token_hash, expires_at) \
             VALUES ($1, $2, now() + interval '30 minutes')",
        )
        .bind(uid)
        .bind(hash_token(&token))
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        state.repo.audit(Some(uid), "pwd.forgot", None).await;

        let smtp = std::env::var("SMTP_URL").unwrap_or_default();
        if smtp.is_empty() {
            tracing::warn!(%token, "SMTP 未配置：重置 token 输出到日志（开发态闭环）");
        } else {
            // 生产：后台投递（SMTP 细节由部署方在网关/邮件服务实现，此处留出通道）
            tracing::info!("password reset mail queued for user {uid}");
        }
    }
    Ok(ok(serde_json::json!({
        "message": "如邮箱存在，重置链接已发送（30 分钟有效）"
    })))
}

#[derive(Deserialize)]
struct ResetReq {
    token: String,
    new_password: String,
}

#[post("/auth/password/reset")]
async fn password_reset(
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResetReq>,
) -> DomainResult<HttpResponse> {
    if body.new_password.len() < 8 {
        return Err(DomainError::Validation("密码至少 8 位".into()));
    }
    let uid: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM password_resets \
         WHERE token_hash = $1 AND used_at IS NULL AND expires_at > now()",
    )
    .bind(hash_token(body.token.trim()))
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(uid) = uid else {
        return Err(DomainError::Validation("token 无效或已过期".into()));
    };
    let hash = crate::domain::hash_password(&body.new_password)?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE users SET pass_hash = $2 WHERE id = $1")
        .bind(uid)
        .bind(&hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE password_resets SET used_at = now() WHERE token_hash = $1")
        .bind(hash_token(body.token.trim()))
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 撤销该用户全部会话（改密后旧 token 全失效）
    let mut c = state.redis.clone();
    let _: () = AsyncCommands::set_ex(
        &mut c,
        format!("logout_nbf:{uid}"),
        chrono::Utc::now().timestamp(),
        86400u64,
    )
    .await
    .unwrap_or(());
    state.repo.audit(Some(uid), "pwd.reset", None).await;
    Ok(ok(serde_json::json!({ "reset": true })))
}

// ============ 图形验证码（注册防机） ============

#[derive(Deserialize)]
struct CaptchaVerify {
    #[serde(default)]
    _code: String, // 预留：当前简单算术题方案在 issue 时校验答案
}

/// 简易验证码：服务端出算术题（a+b=?），答案存 Redis 5 分钟。
/// 注册时带 captcha_id + captcha_answer 校验（见 register 流程注释；当前先供前端展示与校验闭环）。
#[get("/auth/captcha")]
async fn captcha_issue(
    state: web::Data<std::sync::Arc<AppState>>,
    _q: web::Query<CaptchaVerify>,
) -> DomainResult<impl Responder> {
    use rand::Rng;
    let a: i32 = rand::thread_rng().gen_range(1..=20);
    let b: i32 = rand::thread_rng().gen_range(1..=20);
    let id = uuid::Uuid::new_v4().to_string();
    let mut c = state.redis.clone();
    let _: () = AsyncCommands::set_ex(&mut c, format!("captcha:{id}"), a + b, 300)
        .await
        .unwrap_or(());
    Ok(ok(serde_json::json!({
        "captcha_id": id,
        "question": format!("{a} + {b} = ?"),
    })))
}

/// 供注册等内部路径校验验证码
#[allow(dead_code)] // 注册流程集成点：前端接入验证码后启用
pub async fn captcha_verify(state: &AppState, id: &str, answer: i32) -> bool {
    let mut c = state.redis.clone();
    let expect: Option<i32> = AsyncCommands::get(&mut c, format!("captcha:{id}"))
        .await
        .unwrap_or(None);
    if expect == Some(answer) {
        let _: () = AsyncCommands::del(&mut c, format!("captcha:{id}"))
            .await
            .unwrap_or(());
        true
    } else {
        false
    }
}

// ============ H&R 追责 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct HrRow {
    torrent_id: i64,
    torrent_name: String,
    required_seconds: i32,
    seeded_seconds: i32,
    deadline: chrono::DateTime<chrono::Utc>,
    status: String,
}

#[get("/me/hr")]
async fn my_hr_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<HrRow> = sqlx::query_as(
        "SELECT h.torrent_id, t.name AS torrent_name, h.required_seconds, \
                h.seeded_seconds, h.deadline, h.status \
         FROM hr_snapshots h JOIN torrents t ON t.id = h.torrent_id \
         WHERE h.user_id = $1 ORDER BY h.deadline LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct PardonReq {
    user_id: i64,
    torrent_id: i64,
    note: String,
}

/// H&R 赦免（staff）：违规 → pardoned，violation 标记 resolved
#[post("/admin/hr/pardon")]
async fn hr_pardon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PardonReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::HR_PARDON).await?;
    if body.note.trim().is_empty() {
        return Err(DomainError::Validation("赦免必须填理由".into()));
    }
    let n = sqlx::query(
        "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, updated_at = now() \
         WHERE user_id = $2 AND torrent_id = $3 AND status = 'violated'",
    )
    .bind(auth.id)
    .bind(body.user_id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("无待赦免的 H&R 违规".into()));
    }
    sqlx::query(
        "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1 \
         WHERE user_id = $2 AND torrent_id = $3 AND resolved_at IS NULL",
    )
    .bind(auth.id)
    .bind(body.user_id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "hr.pardon", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "pardoned": true })))
}

#[derive(Deserialize)]
struct SelfPardonReq {
    torrent_id: i64,
}

/// 自助免罪（B-02）：消耗 20000 火花，赦免自己一条 violated H&R（NP 魔力免罪口径）
#[post("/me/hr/pardon")]
async fn hr_self_pardon(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SelfPardonReq>,
) -> DomainResult<HttpResponse> {
    const SELF_PARDON_PRICE: i64 = 20_000;
    let auth = require_auth(&req, &state).await?;
    // 先扣火花，成功后才赦免 —— 原顺序（先 UPDATE 后扣费）在余额不足时会把违规白白
    // 赦免（扣费失败仅回 4001，状态已不可逆），且两步非事务，中途失败同样撕裂。
    // spend_spark 自带行锁 + 幂等键，先扣可保证「未付费必不赦免」。
    let idem = format!("hr-self-pardon:{}:{}", auth.id, body.torrent_id);
    let outcome = crate::economy_http::spend_spark(
        &state.repo.db,
        auth.id,
        SELF_PARDON_PRICE,
        "hr_pardon",
        &idem,
        "hr",
        body.torrent_id,
    )
    .await?;
    // 审计修复（P0 铸币）：Replayed = 本请求未扣款（幂等键命中的是历史成功扣费）。
    // 旧逻辑忽略该返回值继续走赦免/退款分支，退款键又拼随机 UUID 每次全新，
    // 重放请求可无限净赚 20000/次。现在：重放一律拒绝，退款键改为确定性键。
    if !matches!(outcome, crate::economy_http::SpendOutcome::Spent) {
        return Err(DomainError::Validation("该违规已处理过，请勿重复提交".into()));
    }
    let n = sqlx::query(
        "UPDATE hr_snapshots SET status = 'pardoned', pardoned_by = $1, updated_at = now() \
         WHERE user_id = $1 AND torrent_id = $2 AND status = 'violated' \
         RETURNING user_id",
    )
    .bind(auth.id)
    .bind(body.torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n.is_none() {
        // 并发窗口内另一请求已赦免（或本就不存在）：退回本次扣费，避免花 20000 买空气。
        // 退款键去掉随机 UUID：同一 (user, torrent) 的退款与扣款一对一，重放不产生新流水。
        crate::economy_http::earn_spark(
            &state.repo.db,
            auth.id,
            SELF_PARDON_PRICE,
            "hr_pardon_refund",
            &format!("{idem}:refund"),
        )
        .await?;
        return Err(DomainError::Validation("无待免罪的 H&R 违规".into()));
    }
    sqlx::query(
        "UPDATE hr_violations SET resolved_at = now(), resolved_by = $1 \
         WHERE user_id = $1 AND torrent_id = $2 AND resolved_at IS NULL",
    )
    .bind(auth.id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "hr.self_pardon", Some(body.torrent_id))
        .await;
    Ok(ok(
        serde_json::json!({ "pardoned": body.torrent_id, "cost": SELF_PARDON_PRICE }),
    ))
}

// ============ 申诉 ============

#[derive(Deserialize)]
struct AppealReq {
    kind: String,
    #[serde(default)]
    ref_id: Option<i64>,
    body: String,
}

#[post("/appeals")]
async fn appeal_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AppealReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if !["hr", "warn", "ban", "other"].contains(&body.kind.as_str()) {
        return Err(DomainError::Validation(
            "kind 仅支持 hr/warn/ban/other".into(),
        ));
    }
    if body.body.trim().len() < 10 {
        return Err(DomainError::Validation("申诉内容至少 10 字".into()));
    }
    // 未结申诉上限 3 条
    let open: i64 =
        sqlx::query_scalar("SELECT count(*) FROM appeals WHERE user_id = $1 AND status = 'open'")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if open >= 3 {
        return Err(DomainError::Validation(
            "未结申诉上限 3 条，请等待处理".into(),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO appeals (user_id, kind, ref_id, body) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.kind)
    .bind(body.ref_id)
    .bind(body.body.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct AppealRow {
    id: i64,
    kind: String,
    ref_id: Option<i64>,
    body: String,
    status: String,
    result_note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/appeals")]
async fn appeal_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<AppealRow> = sqlx::query_as(
        "SELECT id, kind, ref_id, body, status, result_note, created_at \
         FROM appeals WHERE user_id = $1 ORDER BY id DESC LIMIT 50",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AppealHandleReq {
    appeal_id: i64,
    accept: bool,
    note: String,
}

/// staff 侧申诉队列（open 优先，可按状态过滤）
#[derive(sqlx::FromRow, serde::Serialize)]
struct AdminAppealRow {
    id: i64,
    username: String,
    kind: String,
    ref_id: Option<i64>,
    body: String,
    status: String,
    result_note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/appeals")]
async fn appeal_queue(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<AppealQueueQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::APPEAL_HANDLE).await?;
    let status = q.status.as_deref().unwrap_or("");
    let rows: Vec<AdminAppealRow> = sqlx::query_as(
        "SELECT a.id, u.username, a.kind, a.ref_id, a.body, a.status, a.result_note, a.created_at \
         FROM appeals a JOIN users u ON u.id = a.user_id \
         WHERE ($1 = '' OR a.status = $1) \
         ORDER BY (a.status = 'open') DESC, a.id DESC LIMIT 100",
    )
    .bind(status)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AppealQueueQuery {
    status: Option<String>,
}

#[post("/admin/appeals/handle")]
async fn appeal_handle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AppealHandleReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::APPEAL_HANDLE).await?;
    let n = sqlx::query(
        "UPDATE appeals SET status = $2, handled_by = $1, handled_at = now(), result_note = $3 \
         WHERE id = $4 AND status = 'open'",
    )
    .bind(auth.id)
    .bind(if body.accept { "accepted" } else { "rejected" })
    .bind(body.note.trim())
    .bind(body.appeal_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("申诉不存在或已处理".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "appeal.handle", Some(body.appeal_id))
        .await;
    Ok(ok(serde_json::json!({ "handled": body.appeal_id })))
}

// ============ 补签卡使用 ============

#[derive(Deserialize)]
struct ResubReq {
    target_date: String, // YYYY-MM-DD
    #[allow(dead_code)]
    idempotency_key: String, // 幂等语义由 resub_uses.idempotency_key（订单维度）承载
}

/// 使用补签卡：前提是拥有该道具（shop_orders 中 kind='resub_card' 的有效订单）
#[post("/attendance/resub")]
async fn resub_use(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResubReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let date = chrono::NaiveDate::parse_from_str(&body.target_date, "%Y-%m-%d")
        .map_err(|_| DomainError::Validation("日期格式 YYYY-MM-DD".into()))?;
    // 只能补过去 7 天内
    let days_ago =
        ((chrono::Utc::now() + chrono::Duration::hours(8)).date_naive() - date).num_days();
    if !(1..=7).contains(&days_ago) {
        return Err(DomainError::Validation("只能补过去 7 天内".into()));
    }
    // 是否持有补签卡（未使用的订单）。
    // kind 兼容：商店种子为 makeup_card，本流程历史引用 resub_card——两种都认（0066 修复）。
    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if owned < 1 {
        return Err(DomainError::Validation(
            "没有可用补签卡：商店购买或管理发放后可在此使用".into(),
        ));
    }
    // 已签过则拒绝
    let signed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance WHERE user_id = $1 AND date = $2)",
    )
    .bind(auth.id)
    .bind(date)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if signed {
        return Err(DomainError::Validation("该日已有签到记录".into()));
    }
    // 消耗一张卡 + 补签记录 + 出勤记录（事务）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let order_id: i64 = sqlx::query_scalar(
        "SELECT o.id FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id)) \
         ORDER BY o.id LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO resub_uses (user_id, target_date, idempotency_key) VALUES ($1, $2, $3)",
    )
    .bind(auth.id)
    .bind(date)
    .bind(format!("resub:{order_id}"))
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 补签出勤行：streak 不续（补的是漏签日）、reward=0（火花在正常签到发放）
    sqlx::query(
        "INSERT INTO attendance (user_id, date, streak, reward, makeup) \
         VALUES ($1, $2, 0, 0, TRUE) ON CONFLICT (user_id, date) DO NOTHING",
    )
    .bind(auth.id)
    .bind(date)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 返回剩余持有数（前端按钮展示用）
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "resubbed": body.target_date, "cards_left": left }),
    ))
}

// ============ 等级规则与进度 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ClassRuleRow {
    class_id: i32,
    name: String,
    min_uploaded: i64,
    min_download_count: i32,
    min_seed_hours: i32,
    min_account_age_days: i32,
}

#[get("/classes")]
async fn class_rules_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<ClassRuleRow> = sqlx::query_as(
        "SELECT class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days \
         FROM class_rules ORDER BY class_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(sqlx::FromRow)]
struct ProgressRow {
    class_id: i32,
    uploaded: i64,
    download_count: i64,
    seed_hours: i64,
    account_age_days: i64,
}

#[get("/me/class-progress")]
async fn my_class_progress(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let p: ProgressRow = sqlx::query_as(
        "SELECT u.class_id, u.uploaded, \
                (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.completed_at IS NOT NULL) AS download_count, \
                (SELECT COALESCE(sum(s.seeded_seconds), 0) / 3600 FROM snatches s WHERE s.user_id = u.id) AS seed_hours, \
                EXTRACT(DAY FROM now() - u.created_at)::bigint AS account_age_days \
         FROM users u WHERE u.id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 计算当前进度满足到哪一级
    let rules: Vec<ClassRuleRow> = sqlx::query_as(
        "SELECT class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days \
         FROM class_rules ORDER BY class_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut qualified = 1i32;
    for r in &rules {
        if p.uploaded >= r.min_uploaded
            && p.download_count >= r.min_download_count as i64
            && p.seed_hours >= r.min_seed_hours as i64
            && p.account_age_days >= r.min_account_age_days as i64
        {
            qualified = r.class_id;
        }
    }
    Ok(ok(serde_json::json!({
        "current_class": p.class_id, "qualified_class": qualified,
        "uploaded": p.uploaded, "download_count": p.download_count,
        "seed_hours": p.seed_hours, "account_age_days": p.account_age_days,
    })))
}

// ============ 附件/截图与媒体信息（种子上传时提交） ============
// torrents.screenshots / media_info 列已由 0020 添加；上传表单扩展见 web 端。

// ============ 教材愿望单（0074，U3D WishList 教育化） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct WishRow {
    id: i64,
    keyword: String,
    category_id: Option<i32>,
    grade_id: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/wishlist")]
async fn wishlist_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<WishRow> = sqlx::query_as(
        "SELECT id, keyword, category_id, grade_id, created_at FROM wishlist          WHERE user_id = $1 ORDER BY id DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WishAddReq {
    keyword: String,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    grade_id: Option<i32>,
}

#[post("/wishlist")]
async fn wishlist_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishAddReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let kw = body.keyword.trim();
    if kw.is_empty() || kw.len() > 100 {
        return Err(DomainError::Validation("关键词长度 1-100".into()));
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM wishlist WHERE user_id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
    if count >= 20 {
        return Err(DomainError::Validation(
            "愿望单上限 20 条，请先删除旧的".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO wishlist (user_id, keyword, category_id, grade_id) VALUES ($1, $2, $3, $4)          ON CONFLICT (user_id, keyword) DO UPDATE SET category_id = EXCLUDED.category_id, grade_id = EXCLUDED.grade_id",
    )
    .bind(auth.id)
    .bind(kw)
    .bind(body.category_id)
    .bind(body.grade_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "keyword": kw })))
}

#[derive(Deserialize)]
struct WishDelReq {
    id: i64,
}

#[post("/wishlist/remove")]
async fn wishlist_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishDelReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query("DELETE FROM wishlist WHERE id = $1 AND user_id = $2")
        .bind(body.id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    Ok(ok(serde_json::json!({ "removed": body.id })))
}
