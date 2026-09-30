//! 物品目录删除（后台写侧）。
//!
//! 单独成文件是因为删除的**风险面和保存完全不同**：保存要跨池回查 EV，
//! 删除要查「有没有人在引用这件东西、有没有账」。两条逻辑挤在一个文件里
//! 会把行数叠过 300 上限，也容易让人只看见其中一半。

use actix_web::{delete, web, HttpRequest, HttpResponse};
use serde_json::json;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::pool::dberr;

/// 删除目录项。**两道必须先查，否则删除本身就是炸玩法读表的路**：
///  · 还有奖池位引用它 —— `arcade_pool_entries.item_key` 是
///    `ON DELETE SET NULL`，而 CHECK 要求 `kind='item'` 时 item_key 非空，
///    删掉之后 load_pool 会当场校验失败，玩法直接拒绝服务；
///  · 发放账里有它 —— 余量与每人上限都由 `arcade_item_grants` 反推，
///    删物品会 CASCADE 清账，等于把「发出去多少」的证据抹掉。有账只许停用。
#[delete("/admin/arcade/items/{key}")]
pub(super) async fn arcade_item_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let key = path.into_inner();
    let db = &state.repo.db;

    let name: Option<String> =
        sqlx::query_scalar("SELECT name FROM arcade_items WHERE key = $1")
            .bind(&key)
            .fetch_optional(db)
            .await
            .map_err(dberr)?;
    let name = name.ok_or_else(|| {
        DomainError::Validation(format!("目录里没有「{key}」"))
    })?;

    let refs: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT e.label, p.key
          FROM arcade_pool_entries e
          JOIN arcade_pools p ON p.key = e.pool_key
         WHERE e.item_key = $1 AND p.enabled AND e.enabled
         ORDER BY p.key, e.sort
        "#,
    )
    .bind(&key)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    if !refs.is_empty() {
        let spots: Vec<String> = refs
            .iter()
            .map(|(label, pk)| format!("{pk}·{label}"))
            .collect();
        return Err(DomainError::Validation(format!(
            "「{name}」仍被 {} 个奖池位引用（{}）：先从奖池撤下再删",
            refs.len(),
            spots.join("、")
        )));
    }

    let grants: (i64, i64) = sqlx::query_as(
        r#"
        SELECT count(*)::bigint, COALESCE(sum(qty), 0)::bigint
          FROM arcade_item_grants WHERE item_key = $1
        "#,
    )
    .bind(&key)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    if grants.0 > 0 {
        let why = "发放账是余量与每人上限的唯一真相，\
                   删物品会连账一起清掉 —— 请改为停用";
        return Err(DomainError::Validation(format!(
            "「{name}」已发放过 {} 件（{} 条账）。{why}",
            grants.1, grants.0
        )));
    }

    let deleted = sqlx::query("DELETE FROM arcade_items WHERE key = $1")
        .bind(&key)
        .execute(db)
        .await
        .map_err(dberr)?
        .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "arcade.item.delete", None)
        .await;
    Ok(ok(json!({ "key": key, "deleted": deleted })))
}
