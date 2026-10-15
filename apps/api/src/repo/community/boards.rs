//! 版块看板行（GET /forums/boards）的 SQL 唯一居所。
//!
//! 2026-10-16 下沉：见 follows.rs 头注。口径与 forum_list 的版块过滤
//! 一致：只回当前用户可读的版块，不泄漏隐藏版块名。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 版块精简行（移动主题下拉 / 首页看板用）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct BoardBrief {
    pub id: i64,
    pub name: String,
    pub category_name: Option<String>,
}

/// 全站版块精简列表。
///
/// `class_id` 是读者的权限级，`user_id` 用于「自己当版主」的可见性豁免，
/// `is_staff` 让全站管理组无视版块可见性。
pub async fn list_boards(
    repo: &Repo,
    user_id: i64,
    class_id: i32,
    is_staff: bool,
) -> DomainResult<Vec<BoardBrief>> {
    Ok(sqlx::query_as(
        "SELECT f.id, f.name, c.name AS category_name \
         FROM forums f LEFT JOIN forum_categories c ON c.id = f.category_id \
         WHERE (f.minclassread <= $1 OR EXISTS ( \
                 SELECT 1 FROM forum_mods fm \
                  WHERE fm.forum_id = f.id AND fm.user_id = $2)) \
           AND (c.id IS NULL OR c.visible = TRUE OR $3) \
         ORDER BY c.sort NULLS LAST, c.id NULLS LAST, f.sort, f.id",
    )
    .bind(class_id)
    .bind(user_id)
    .bind(is_staff)
    .fetch_all(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}
