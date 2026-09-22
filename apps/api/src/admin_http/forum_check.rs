//! 版块写入校验（从 forum_mods.rs 拆出，防单文件超 300 行）。
//! 契约：`forum_upsert_check` 返回**归一化后的名称**，调用方必须用返回值落库，
//! 否则用户输入的首尾空白与连续空白会原样入库（渲染与搜索都会受影响）。
//! 门槛值域 0..=99 与 `user_classes` 的 class 上限一致。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::forum_mods::ForumUpsertReq;

/// 名称归一化：trim + 折叠连续空白（含全角空格）
pub fn norm_name(raw: &str) -> String {
    let cleaned = raw.replace('\u{3000}', " ");
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 版块写入前置校验；成功时返回归一化后的名称。
pub fn forum_upsert_check(body: &ForumUpsertReq) -> DomainResult<String> {
    let name = norm_name(&body.name);
    if name.is_empty() {
        return Err(DomainError::Validation("版块名不能为空".into()));
    }
    if name.chars().count() > 64 {
        return Err(DomainError::Validation("版块名最长 64 字".into()));
    }
    let mr = body.minclassread.unwrap_or(0);
    let mw = body.minclasswrite.unwrap_or(0);
    let mc = body.minclasscreate.unwrap_or(0);
    for (v, label) in [(mr, "读"), (mw, "回"), (mc, "发")] {
        if !(0..=99).contains(&v) {
            return Err(DomainError::Validation(format!(
                "门槛值需在 0..99 之间（{label} 门槛 = {v}）"
            )));
        }
    }
    // NP 口径：读 ≤ 回 ≤ 发。0 表示所有人。
    if !(mr <= mw && mw <= mc) {
        return Err(DomainError::Validation(
            "三档门槛需满足 读 ≤ 回 ≤ 发".into(),
        ));
    }
    Ok(name)
}

/// 归属分区存在性校验（None = 不分组，放行）
pub async fn ensure_category_exists(
    db: &PgPool,
    category_id: Option<i64>,
) -> DomainResult<()> {
    let Some(cid) = category_id else {
        return Ok(());
    };
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM forum_categories WHERE id = $1)",
    )
    .bind(cid)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        return Err(DomainError::Validation(format!(
            "指定的分区不存在（id={cid}）"
        )));
    }
    Ok(())
}

/// 唯一约束冲突 → 友好校验错误；其余原样转内部错误。
/// 0154 给 `forum_categories` 加了 `uq_forum_categories_name`（lower+btrim），
/// 若不映射，用户建重名分区会看到「内部错误 500」而不是可读提示。
pub fn unique_or_internal(e: sqlx::Error, name: &str) -> DomainError {
    let dup = matches!(&e, sqlx::Error::Database(db)
        if db.code().as_deref() == Some("23505"));
    if dup {
        DomainError::Validation(format!("已存在同名分区「{name}」"))
    } else {
        DomainError::Internal(e.into())
    }
}

/// 读出版块当前三档门槛（用于更新时比对是否被清空）
pub async fn current_gates(
    db: &PgPool,
    forum_id: i64,
) -> DomainResult<Option<(i32, i32, i32)>> {
    let row: Option<(i32, i32, i32)> = sqlx::query_as(
        "SELECT minclassread, minclasswrite, minclasscreate \
         FROM forums WHERE id = $1",
    )
    .bind(forum_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(row)
}
