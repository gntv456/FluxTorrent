//! 注册答题闸（0372 M5）：站规测验抽题 + 一次性票据判分。
//!
//! 动机：邀请制/开放注册拦的是「谁能进」，拦不住「进来的人不懂规矩」。
//! 注册前答对一题站规常识（分享率/H&R/保种/作弊红线），让新用户在第一步
//! 就被迫读到最关键的几条，同时把无脑注册机的成本再抬一档。
//!
//! 协议（与 captcha/pow 同构，票据一次性）：
//!   ① `GET /auth/quiz-challenge` —— 从启用题库随机抽一题，票据存 Redis
//!      `sha3/quiz:{ticket}` = `{question_id}:{answer_index}`，TTL 走
//!      site_settings.reg_quiz_ttl_minutes（默认 10 分钟）；只回选项文本，
//!      **不回答案**（答案键只在 Redis）；
//!   ② 注册体带 quiz_ticket / quiz_answer，GETDEL 一次性取回后比对下标；
//!   ③ 结果落 reg_quiz_attempts（IP 只存 sha3 摘要）。
//!
//! 开关：reg_quiz_enabled=yes 才生效；reg_quiz_pass_required=no 时只记录
//! 不拦截（先把题库跑起来看通过率，再决定是否硬拦）。

use actix_web::{get, web, HttpRequest, Responder};
use redis::AsyncCommands;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// Redis 键前缀（票据一次性，GETDEL 消费）
const QUIZ_KEY: &str = "sha3/quiz:";

#[derive(sqlx::FromRow)]
struct QuizQuestion {
    id: i64,
    question: String,
    options: serde_json::Value,
    answer_index: i32,
}

fn setting_default(v: Option<String>, d: i64) -> i64 {
    v.and_then(|s| s.trim().parse::<i64>().ok())
        .unwrap_or(d)
        .clamp(1, 1440)
}

/// GET /auth/quiz-challenge：抽题签发一次性票据（不泄露答案）。
#[get("/auth/quiz-challenge")]
pub async fn quiz_challenge(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    // 与 captcha 同口径的 IP 限流：抽题接口本身也要付成本
    let ip = crate::http::client_ip(&req);
    crate::http::rate_window(&state, &format!("quiz:{ip}"), 30, 60).await?;
    let row: Option<QuizQuestion> = sqlx::query_as(
        "SELECT id, question, options, answer_index \
         FROM reg_quiz_questions \
         WHERE enabled \
           AND ($1 = '' OR category = $1) \
         ORDER BY random() LIMIT 1",
    )
    .bind(quiz_category(&state).await)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(q) = row else {
        return Err(DomainError::Validation(
            "题库为空，请联系管理组".into(),
        ));
    };
    let ttl = setting_default(
        read_setting(&state, "reg_quiz_ttl_minutes").await,
        10,
    );
    let ticket = uuid::Uuid::new_v4().to_string();
    let mut c = state.redis.clone();
    let val = format!("{}:{}", q.id, q.answer_index);
    let _: () = AsyncCommands::set_ex(
        &mut c,
        format!("{QUIZ_KEY}{ticket}"),
        val,
        (ttl * 60) as u64,
    )
    .await
    .map_err(|e| {
        DomainError::Internal(anyhow::anyhow!("答题票据存储不可用: {e}"))
    })?;
    let expires_at =
        chrono::Utc::now() + chrono::Duration::minutes(ttl);
    Ok(ok(serde_json::json!({
        "ticket": ticket,
        "question_id": q.id,
        "question": q.question,
        "options": q.options,
        "ttl_minutes": ttl,
        "expires_at": expires_at.to_rfc3339(),
    })))
}

async fn read_setting(state: &AppState, name: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
        .bind(name)
        .fetch_optional(&state.repo.db)
        .await
        .ok()
        .flatten()
}

pub(crate) async fn quiz_category(state: &AppState) -> String {
    read_setting(state, "reg_quiz_category")
        .await
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// 答题结果（register.rs 据此决定是否拦截）。
pub(crate) struct QuizOutcome {
    pub passed: bool,
    pub question_id: i64,
}

/// 校验票据（一次性）：GETDEL 取回 → 比对下标 → 记留痕。
/// 票据不存在/过期 → Err（调用方按「未答题」处理）。
/// `require_pass=false` 时答错也返回 Ok(passed=false)，由调用方决定是否放行。
pub(crate) async fn quiz_verify(
    state: &std::sync::Arc<AppState>,
    ticket: &str,
    answer: i32,
    ip: &str,
) -> DomainResult<QuizOutcome> {
    // 形状闸：票据是 UUID，别把任意串塞进 Redis 键名
    let t = ticket.trim();
    if t.is_empty()
        || t.len() > 64
        || !t
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(DomainError::Validation("答题票据形状无效".into()));
    }
    let mut c = state.redis.clone();
    let val: Option<String> = redis::cmd("GETDEL")
        .arg(format!("{QUIZ_KEY}{t}"))
        .query_async(&mut c)
        .await
        .map_err(|e| {
            DomainError::Internal(anyhow::anyhow!("答题票据存储不可用: {e}"))
        })?;
    let Some(v) = val else {
        return Err(DomainError::Validation(
            "答题票据无效或已过期，请重新抽题".into(),
        ));
    };
    let Some((qid, ans)) = v.split_once(':') else {
        return Err(DomainError::Validation("答题票据损坏".into()));
    };
    let question_id: i64 = qid
        .parse()
        .map_err(|_| DomainError::Validation("答题票据损坏".into()))?;
    let right: i32 = ans
        .parse()
        .map_err(|_| DomainError::Validation("答题票据损坏".into()))?;
    let passed = answer == right;
    // 留痕（失败只告警不阻断注册主流程）
    let _ = sqlx::query(
        "INSERT INTO reg_quiz_attempts \
           (ip_hash, question_id, passed, answer_index) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(crate::auth_http::register_gates::ip_hash(ip))
    .bind(question_id)
    .bind(passed)
    .bind(answer)
    .execute(&state.repo.db)
    .await
    .map_err(|e| tracing::warn!(%e, "reg_quiz_attempts 留痕失败"));
    Ok(QuizOutcome { passed, question_id })
}
