//! 版块配置的版本戳读（`admin_http/forum_mods.rs` 的 SQL 唯一居所）。
//! 2026-10-15 下沉：FRAME §2「H 不 import sqlx」——乐观并发的版本戳
//! 比对是 0376 新加的 H 层 SQL，属新增而非存量，按「新文件零容忍」处理。
//!
//! 为什么要 FOR UPDATE：并发窗口在「读戳」到「UPDATE」之间。若不加锁，
//! 两个带着同一版本戳的请求会先后通过校验、先后写成功，后写者照样
//! 覆盖前者——乐观并发就白做了。行锁把窗口收窄到本事务内。

use super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 当前 `mtime`（None = 版块不存在）。
pub async fn mtime(
    repo: &Repo,
    forum_id: i64,
) -> Option<chrono::DateTime<chrono::Utc>> {
    sqlx::query_scalar("SELECT mtime FROM forums WHERE id = $1 FOR UPDATE")
        .bind(forum_id)
        .fetch_optional(&repo.db)
        .await
        .map_err(|e| crate::errors::DomainError::Internal(e.into()))
        .unwrap_or(None)
}

/// 新建版块的入参（值已由 H 层校验/归一：名称 trim+折叠、门槛值域、
/// 读 ≤ 回 ≤ 发、分区存在性）。
pub struct ForumCreate<'a> {
    pub name: &'a str,
    pub descr: Option<&'a str>,
    /// 版块规则正文（0392）：None / 空串 = 未设置
    pub rules: Option<&'a str>,
    pub mr: i32,
    pub mw: i32,
    pub mc: i32,
    pub protected: bool,
    pub category_id: Option<i64>,
    pub sort: i32,
}

/// 建版。`min_class` 历史上与 `minclassread` 同值（NP 口径），故复用 $3。
pub async fn create_forum(
    repo: &Repo,
    p: ForumCreate<'_>,
) -> DomainResult<i64> {
    sqlx::query_scalar::<_, i64>(
        "INSERT INTO forums (name, descr, rules, minclassread, \
            minclasswrite, minclasscreate, min_class, protected, \
            category_id, sort) \
         VALUES ($1, $2, $9, $3, $4, $5, $3, $6, $7, $8) \
         RETURNING id",
    )
    .bind(p.name)
    .bind(p.descr)
    .bind(p.mr)
    .bind(p.mw)
    .bind(p.mc)
    .bind(p.protected)
    .bind(p.category_id)
    .bind(p.sort)
    .bind(p.rules)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 整条更新的入参（同 ForumCreate，另带行 id）。
pub struct ForumUpdate<'a> {
    pub name: &'a str,
    pub descr: Option<&'a str>,
    /// 版块规则（0392）：**None = 不动这一列**（老客户端 PUT 不会把
    /// 已写好的规则抹掉）；Some("") = 版主清空，如实写空串。
    pub rules: Option<&'a str>,
    pub mr: i32,
    pub mw: i32,
    pub mc: i32,
    pub protected: bool,
    pub category_id: Option<i64>,
    pub sort: i32,
    pub fid: i64,
}

/// 写侧的乐观并发只到这里为止：`expected_mtime` 的比对在 H 层
/// （要先 `mtime()` 取行锁再判），这里只负责把 `mtime` 推进。
pub async fn update_forum(
    repo: &Repo,
    p: ForumUpdate<'_>,
) -> DomainResult<u64> {
    Ok(sqlx::query(
        "UPDATE forums SET name = $1, descr = $2, \
            minclassread = $3, minclasswrite = $4, minclasscreate = $5, \
            min_class = $3, protected = $6, category_id = $7, sort = $8, \
            rules = COALESCE($10, rules), mtime = now() \
         WHERE id = $9",
    )
    .bind(p.name)
    .bind(p.descr)
    .bind(p.mr)
    .bind(p.mw)
    .bind(p.mc)
    .bind(p.protected)
    .bind(p.category_id)
    .bind(p.sort)
    .bind(p.fid)
    .bind(p.rules)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}
