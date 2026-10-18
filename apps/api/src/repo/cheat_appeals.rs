//! 作弊申诉域（0369 M3）的 SQL 唯一居所（FRAME §2 H 层下沉）：
//! 消费方 gaps_http/cheat_appeals.rs（用户侧提交/查询）与
//! gaps_http/cheat_appeal_review.rs（staff 队列/复核）。业务校验
//! （令牌形状、内容长度、MAX_OPEN、四眼、outcomes 白名单）留在 H 层。

use sqlx::{Postgres, Transaction};

use super::Repo;

// ---- 用户侧（cheat_appeals.rs 消费） --------------------------------------

/// 一次性令牌摘要点查：返回持有人 uid；过期/已焚（NULL）自然落空。
pub async fn user_id_by_token_hash(
    repo: &Repo,
    hash: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM users WHERE appeal_token_hash = $1 \
           AND appeal_token_expires_at > now()",
    )
    .bind(hash)
    .fetch_optional(&repo.db)
    .await
}

/// 案件归属核对（防令牌跨用户 / 防冒名申诉）。
pub async fn case_owned_by_user(
    repo: &Repo,
    case_id: i64,
    uid: i64,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM reports \
         WHERE id = $1 AND ref_type = 'cheat' AND ref_id = $2)",
    )
    .bind(case_id)
    .bind(uid)
    .fetch_one(&repo.db)
    .await
}

/// 未结申诉计数（MAX_OPEN 上限判据）。
pub async fn open_appeal_count(
    repo: &Repo,
    uid: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT count(*) FROM appeals WHERE user_id = $1 AND status = 'open'",
    )
    .bind(uid)
    .fetch_one(&repo.db)
    .await
}

/// 令牌路径建申诉 + 用后即焚（单事务：插入与焚令牌同生共死）。
pub async fn create_token_appeal(
    repo: &Repo,
    uid: i64,
    case_id: i64,
    body: &str,
    token_hash: &str,
) -> Result<i64, sqlx::Error> {
    let mut tx = repo.db.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO appeals (user_id, kind, case_id, body, token_hash) \
         VALUES ($1, 'cheat', $2, $3, $4) RETURNING id",
    )
    .bind(uid)
    .bind(case_id)
    .bind(body)
    .bind(token_hash)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE users SET appeal_token_hash = NULL, \
         appeal_token_expires_at = NULL WHERE id = $1",
    )
    .bind(uid)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// 登录路径建申诉（无令牌，不需要焚）。
pub async fn create_appeal(
    repo: &Repo,
    uid: i64,
    case_id: i64,
    body: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO appeals (user_id, kind, case_id, body) \
         VALUES ($1, 'cheat', $2, $3) RETURNING id",
    )
    .bind(uid)
    .bind(case_id)
    .bind(body)
    .fetch_one(&repo.db)
    .await
}

/// 我的申诉行（含结论）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct MyCheatAppeal {
    pub id: i64,
    pub case_id: Option<i64>,
    pub body: String,
    pub status: String,
    pub outcome: Option<String>,
    pub result_note: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn my_appeals(
    repo: &Repo,
    uid: i64,
) -> Result<Vec<MyCheatAppeal>, sqlx::Error> {
    sqlx::query_as::<_, MyCheatAppeal>(
        "SELECT a.id, a.case_id, a.body, a.status, a.outcome, \
                a.result_note, a.created_at \
         FROM appeals a WHERE a.user_id = $1 AND a.kind = 'cheat' \
         ORDER BY a.id DESC LIMIT 50",
    )
    .bind(uid)
    .fetch_all(&repo.db)
    .await
}

/// 用户名点查（查无此人给 None；通知署名由调用方兜底）。
pub async fn username_of(repo: &Repo, uid: i64) -> Option<String> {
    sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
        .bind(uid)
        .fetch_optional(&repo.db)
        .await
        .ok()
        .flatten()
}

/// 管理组信箱立项（permission='cheater' 面；失败由调用方忽略）。
pub async fn insert_staffmessage(
    repo: &Repo,
    uid: i64,
    subject: String,
    body: String,
) -> Result<u64, sqlx::Error> {
    let n = sqlx::query(
        "INSERT INTO staffmessages (user_id, subject, body, permission) \
         VALUES ($1, $2, $3, 'cheater')",
    )
    .bind(uid)
    .bind(subject)
    .bind(body)
    .execute(&repo.db)
    .await?
    .rows_affected();
    Ok(n)
}

// ---- staff 侧（cheat_appeal_review.rs 消费） -------------------------------

/// 队列行（join 出用户名与案件 sanction；by_token 由 token_hash 推导）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct CheatAppealRow {
    pub id: i64,
    pub user_id: i64,
    pub username: String,
    pub case_id: Option<i64>,
    pub body: String,
    pub status: String,
    pub outcome: Option<String>,
    pub result_note: Option<String>,
    pub by_token: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub sanction: Option<serde_json::Value>,
}

pub async fn appeal_queue(
    repo: &Repo,
    status: &str,
) -> Result<Vec<CheatAppealRow>, sqlx::Error> {
    sqlx::query_as::<_, CheatAppealRow>(
        "SELECT a.id, a.user_id, u.username, a.case_id, a.body, a.status, \
                a.outcome, a.result_note, \
                (a.token_hash IS NOT NULL) AS by_token, a.created_at, \
                r.sanction \
         FROM appeals a \
         JOIN users u ON u.id = a.user_id \
         LEFT JOIN reports r ON r.id = a.case_id \
         WHERE a.kind = 'cheat' AND ($1 = '' OR a.status = $1) \
         ORDER BY (a.status = 'open') DESC, a.id DESC LIMIT 100",
    )
    .bind(status)
    .fetch_all(&repo.db)
    .await
}

/// 复核目标点查：(user_id, case_id, sanction)；仅 open 态可复核。
pub async fn resolve_target(
    repo: &Repo,
    id: i64,
) -> Result<Option<(i64, Option<i64>, Option<serde_json::Value>)>, sqlx::Error>
{
    sqlx::query_as(
        "SELECT a.user_id, a.case_id, r.sanction \
         FROM appeals a LEFT JOIN reports r ON r.id = a.case_id \
         WHERE a.id = $1 AND a.kind = 'cheat' AND a.status = 'open'",
    )
    .bind(id)
    .fetch_optional(&repo.db)
    .await
}

/// 结论落库（调用方持有事务与 commit）。
pub async fn resolve_appeal(
    tx: &mut Transaction<'_, Postgres>,
    id: i64,
    status: &str,
    outcome: &str,
    handled_by: i64,
    note: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE appeals SET status = $1, outcome = $2, handled_by = $3, \
         handled_at = now(), result_note = $4 WHERE id = $5",
    )
    .bind(status)
    .bind(outcome)
    .bind(handled_by)
    .bind(note)
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 案件备注（review 维持/减轻分支与 reverse 推翻分支共用）。
pub async fn insert_case_note(
    tx: &mut Transaction<'_, Postgres>,
    case_id: i64,
    author_id: i64,
    body: String,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO case_notes (case_id, author_id, body) \
         VALUES ($1, $2, $3)",
    )
    .bind(case_id)
    .bind(author_id)
    .bind(body)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// 当事人通知目标：(username, email)。
pub async fn username_email_of(
    repo: &Repo,
    uid: i64,
) -> Option<(String, Option<String>)> {
    sqlx::query_as("SELECT username, email FROM users WHERE id = $1")
        .bind(uid)
        .fetch_optional(&repo.db)
        .await
        .ok()
        .flatten()
}

/// 站名（邮件标题用；缺省 FluxTorrent）。
pub async fn site_name(repo: &Repo) -> String {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'site_name'), 'FluxTorrent')",
    )
    .fetch_one(&repo.db)
    .await
    .unwrap_or_else(|_| "FluxTorrent".into())
}
