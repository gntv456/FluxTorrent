//! 注册答题闸（0372 M5）的 SQL 唯一居所（FRAME §2 H 层下沉）：
//! 消费方 gaps_http/reg_quiz.rs、admin_http/reg_quiz_admin.rs、
//! admin_http/reg_quiz_bank.rs。业务校验（validate_quiz）留在 H 层。

use super::Repo;

/// 抽题行（题面 + 答案键；答案只进 Redis 不回前端）。
#[derive(sqlx::FromRow)]
pub struct QuizQuestion {
    pub id: i64,
    pub question: String,
    pub options: serde_json::Value,
    pub answer_index: i32,
}

/// 题库管理行（admin 列表；序列化给管理端，含 answer_index 不外泄游客）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct QuizRow {
    pub id: i64,
    pub category: String,
    pub question: String,
    pub options: serde_json::Value,
    pub answer_index: i32,
    pub explanation: Option<String>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 新增/导入/重写共用的落库形状（校验后的干净数据）。
pub struct NewQuestion {
    pub category: String,
    pub question: String,
    pub options: serde_json::Value,
    pub answer_index: i32,
    pub explanation: Option<String>,
    pub enabled: bool,
}

/// 从启用题库随机抽一题（category 空串 = 全库）。
pub async fn random_question(
    repo: &Repo,
    category: &str,
) -> Result<Option<QuizQuestion>, sqlx::Error> {
    sqlx::query_as::<_, QuizQuestion>(
        "SELECT id, question, options, answer_index \
         FROM reg_quiz_questions \
         WHERE enabled \
           AND ($1 = '' OR category = $1) \
         ORDER BY random() LIMIT 1",
    )
    .bind(category)
    .fetch_optional(&repo.db)
    .await
}

/// site_settings 单键文本读（缺行/失败都给 None，调用方自带默认值）。
pub async fn read_setting(repo: &Repo, name: &str) -> Option<String> {
    sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
        .bind(name)
        .fetch_optional(&repo.db)
        .await
        .ok()
        .flatten()
}

/// 答题留痕（失败由调用方告警、不阻断注册主流程）。
pub async fn record_attempt(
    repo: &Repo,
    ip_hash: &str,
    question_id: i64,
    passed: bool,
    answer_index: i32,
) -> Result<u64, sqlx::Error> {
    let n = sqlx::query(
        "INSERT INTO reg_quiz_attempts \
           (ip_hash, question_id, passed, answer_index) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(ip_hash)
    .bind(question_id)
    .bind(passed)
    .bind(answer_index)
    .execute(&repo.db)
    .await?
    .rows_affected();
    Ok(n)
}

/// 题库列表（管理端全量，enabled 优先）。
pub async fn list_questions(
    repo: &Repo,
    limit: i64,
) -> Result<Vec<QuizRow>, sqlx::Error> {
    sqlx::query_as::<_, QuizRow>(
        "SELECT id, category, question, options, answer_index, \
                explanation, enabled, created_at \
         FROM reg_quiz_questions ORDER BY enabled DESC, category, id \
         LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&repo.db)
    .await
}

/// 近 N 天答题量与通过量（(attempts, passed)；无数据 (0,0) 由调用方兜）。
pub async fn attempt_stats(
    repo: &Repo,
    days: i64,
) -> Result<(i64, i64), sqlx::Error> {
    sqlx::query_as::<_, (i64, i64)>(
        "SELECT count(*), count(*) FILTER (WHERE passed) \
         FROM reg_quiz_attempts \
         WHERE created_at > now() - ($1::bigint * interval '1 day')",
    )
    .bind(days)
    .fetch_one(&repo.db)
    .await
}

/// 题库总数。
pub async fn count_questions(repo: &Repo) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM reg_quiz_questions")
        .fetch_one(&repo.db)
        .await
}

/// 新增一题，返回 id。
pub async fn add_question(
    repo: &Repo,
    q: NewQuestion,
    created_by: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO reg_quiz_questions \
           (category, question, options, answer_index, explanation, \
            enabled, created_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(q.category)
    .bind(q.question)
    .bind(q.options)
    .bind(q.answer_index)
    .bind(q.explanation)
    .bind(q.enabled)
    .bind(created_by)
    .fetch_one(&repo.db)
    .await
}

/// 整题重写（enabled 走 COALESCE：None = 保持原启用态）。
pub async fn update_full(
    repo: &Repo,
    q: NewQuestion,
    enabled: Option<bool>,
    id: i64,
) -> Result<u64, sqlx::Error> {
    let n = sqlx::query(
        "UPDATE reg_quiz_questions SET category = $1, question = $2, \
           options = $3, answer_index = $4, explanation = $5, \
           enabled = COALESCE($6, enabled) WHERE id = $7",
    )
    .bind(q.category)
    .bind(q.question)
    .bind(q.options)
    .bind(q.answer_index)
    .bind(q.explanation)
    .bind(enabled)
    .bind(id)
    .execute(&repo.db)
    .await?
    .rows_affected();
    Ok(n)
}

/// 只改启用态。
pub async fn update_enabled(
    repo: &Repo,
    enabled: bool,
    id: i64,
) -> Result<u64, sqlx::Error> {
    let n =
        sqlx::query("UPDATE reg_quiz_questions SET enabled = $1 WHERE id = $2")
            .bind(enabled)
            .bind(id)
            .execute(&repo.db)
            .await?
            .rows_affected();
    Ok(n)
}

/// 批量删除，返回删除数。
pub async fn delete_questions(
    repo: &Repo,
    ids: &[i64],
) -> Result<u64, sqlx::Error> {
    let n = sqlx::query("DELETE FROM reg_quiz_questions WHERE id = ANY($1)")
        .bind(ids)
        .execute(&repo.db)
        .await?
        .rows_affected();
    Ok(n)
}

/// 幂等导入（同题干跳过）：单事务逐条 INSERT..WHERE NOT EXISTS，
/// 返回实际新增数。非法项的收集在 H 层完成，这里只收干净行。
pub async fn import_questions(
    repo: &Repo,
    items: Vec<NewQuestion>,
    created_by: i64,
) -> Result<i64, sqlx::Error> {
    let mut tx = repo.db.begin().await?;
    let mut added = 0i64;
    for q in items {
        let n = sqlx::query(
            "INSERT INTO reg_quiz_questions \
               (category, question, options, answer_index, explanation, \
                enabled, created_by) \
             SELECT $1, $2, $3, $4, $5, $6, $7 \
             WHERE NOT EXISTS ( \
                   SELECT 1 FROM reg_quiz_questions WHERE question = $2)",
        )
        .bind(q.category)
        .bind(&q.question)
        .bind(q.options)
        .bind(q.answer_index)
        .bind(q.explanation)
        .bind(q.enabled)
        .bind(created_by)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        added += n as i64;
    }
    tx.commit().await?;
    Ok(added)
}
