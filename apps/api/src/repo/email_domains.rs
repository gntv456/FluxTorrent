//! 一次性邮箱域名库（0370 M4）的 SQL 唯一居所（FRAME §2 H 层下沉）：
//! 消费方 admin_http/email_domains.rs。归一化/校验（normalize）与
//! 明细统计留在 H 层；repo 只收干净域名。

use super::Repo;

/// 列表行（source: builtin | import）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct DomainRow {
    pub domain: String,
    pub source: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 列表（q 子串匹配，LIKE 由调用方拼 %%）。
pub async fn list_domains(
    repo: &Repo,
    needle: &str,
    limit: i64,
) -> Result<Vec<DomainRow>, sqlx::Error> {
    sqlx::query_as::<_, DomainRow>(
        "SELECT domain, source, created_at FROM disposable_email_domains \
         WHERE domain LIKE $1 ORDER BY source, domain LIMIT $2",
    )
    .bind(needle)
    .bind(limit)
    .fetch_all(&repo.db)
    .await
}

/// 全库计数。
pub async fn count_domains(repo: &Repo) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM disposable_email_domains")
        .fetch_one(&repo.db)
        .await
}

/// 导出：纯域名数组（可分享 JSON 用）。
pub async fn export_domains(repo: &Repo) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT domain FROM disposable_email_domains ORDER BY domain",
    )
    .fetch_all(&repo.db)
    .await
}

/// 幂等导入（ON CONFLICT DO NOTHING）：整批一事务，返回实际新增数。
/// 上限校验在 H 层（IMPORT_MAX）。
pub async fn import_domains(
    repo: &Repo,
    domains: &[String],
) -> Result<i64, sqlx::Error> {
    let mut tx = repo.db.begin().await?;
    let mut added = 0i64;
    for d in domains {
        let n = sqlx::query(
            "INSERT INTO disposable_email_domains (domain, source) \
             VALUES ($1, 'import') ON CONFLICT (domain) DO NOTHING",
        )
        .bind(d)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        added += n as i64;
    }
    tx.commit().await?;
    Ok(added)
}

/// 按域名批量删除，返回删除数。
pub async fn delete_domains(
    repo: &Repo,
    domains: &[String],
) -> Result<u64, sqlx::Error> {
    let n = sqlx::query(
        "DELETE FROM disposable_email_domains WHERE domain = ANY($1)",
    )
    .bind(domains)
    .execute(&repo.db)
    .await?
    .rows_affected();
    Ok(n)
}
