//! 注册治理四道闸（0370 M4）的 SQL 唯一居所（FRAME §2 H 层下沉）：
//! 消费方 auth_http/register_gates.rs（闸逻辑/JSON 解析/提示文案留在
//! H 层）与 register.rs 的建号留痕。fail-open 语义（查询失败不阻断）
//! 由 H 层 unwrap_or 兜底，repo 只如实返回结果。

use super::Repo;

/// site_settings 单键文本读（缺行/失败给 None，调用方自带默认值）。
pub async fn setting(repo: &Repo, name: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
        .bind(name)
        .fetch_optional(&repo.db)
        .await
        .ok()
        .flatten()
}

/// 窗口名额口径：since 起已建账号数（含已封禁——名额是发号量）。
pub async fn users_created_since(
    repo: &Repo,
    since: chrono::DateTime<chrono::Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM users WHERE created_at >= $1")
        .bind(since)
        .fetch_one(&repo.db)
        .await
}

/// 开放注册名额口径（P-684）：只数**直接注册**（invited_by IS NULL）——
/// 邀请码注册走的是邀请人自己的配额，不应消耗开放轮名额；站长设定的
/// 「开放 100 人」就是 100 个免邀请码入口。
pub async fn direct_signups_since(
    repo: &Repo,
    since: chrono::DateTime<chrono::Utc>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM users          WHERE created_at >= $1 AND invited_by IS NULL",
    )
    .bind(since)
    .fetch_one(&repo.db)
    .await
}

/// 同 IP 闸：窗口小时内该 IP 摘要的注册数。
pub async fn ip_signup_count(
    repo: &Repo,
    ip_hash: &str,
    hours: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM reg_ip_signups \
         WHERE ip_hash = $1 \
           AND created_at > now() - ($2::bigint * interval '1 hour')",
    )
    .bind(ip_hash)
    .bind(hours)
    .fetch_one(&repo.db)
    .await
}

/// 一次性邮箱命中（含子域后缀匹配：`$1 LIKE '%.' || domain`）。
pub async fn disposable_hit(
    repo: &Repo,
    domain: &str,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM disposable_email_domains \
         WHERE domain = $1 OR $1 LIKE '%.' || domain)",
    )
    .bind(domain)
    .fetch_one(&repo.db)
    .await
}

/// 建号留痕（附属数据；失败由调用方告警不阻断注册）。
pub async fn insert_ip_signup(
    repo: &Repo,
    ip_hash: &str,
    user_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO reg_ip_signups (ip_hash, user_id) VALUES ($1, $2)",
    )
    .bind(ip_hash)
    .bind(user_id)
    .execute(&repo.db)
    .await?;
    Ok(())
}
