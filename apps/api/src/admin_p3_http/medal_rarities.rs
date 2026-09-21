//! P2-7 勋章稀有度 CRUD
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[derive(sqlx::FromRow, serde::Serialize)]
struct MedalRarityAdminRow {
    value: String,
    label: String,
    tone: String,
    sort: i32,
    /// 当前有多少枚勋章在用这个稀有度（删除前的安全提示）
    used: i64,
}

#[derive(Deserialize)]
struct MedalRarityReq {
    /// 新增时必填（英文 slug，落 medals.rarity）；编辑时作为「改键」用，省略则不改键
    #[serde(default)]
    value: Option<String>,
    label: String,
    #[serde(default)]
    tone: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
}

/// slug 校验：只允许小写字母/数字/下划线/短横线（它会进 medals.rarity 并出现在 URL 上）
pub(super) fn clean_rarity_value(raw: &str) -> String {
    raw.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(32)
        .collect()
}

#[get("/admin/medal-rarities")]
async fn admin_medal_rarities(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    let rows: Vec<MedalRarityAdminRow> = sqlx::query_as(
        "SELECT r.value, r.label, r.tone, r.sort, \
                (SELECT count(*) FROM medals m WHERE m.rarity = r.value)::bigint AS used \
         FROM medal_rarities r ORDER BY r.sort, r.value",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[post("/admin/medal-rarities")]
async fn admin_medal_rarity_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MedalRarityReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    let value = clean_rarity_value(body.value.as_deref().unwrap_or(""));
    if value.is_empty() {
        return Err(DomainError::Validation(
            "稀有度键不能为空（仅字母/数字/下划线/短横线）".into(),
        ));
    }
    let label = body.label.trim();
    if label.is_empty() {
        return Err(DomainError::Validation("显示名不能为空".into()));
    }
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM medal_rarities WHERE value = $1)",
    )
    .bind(&value)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dup {
        return Err(DomainError::Validation(format!("稀有度 {value} 已存在")));
    }
    sqlx::query(
        "INSERT INTO medal_rarities (value, label, tone, sort) VALUES \
         ($1, $2, COALESCE($3, 'sky'), COALESCE($4, 100))",
    )
    .bind(&value)
    .bind(label)
    .bind(body.tone.clone())
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "medal_rarity.add", None)
        .await;
    Ok(ok(serde_json::json!({ "value": value })))
}

#[put("/admin/medal-rarities/{value}")]
async fn admin_medal_rarity_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<MedalRarityReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    let old = path.into_inner();
    let label = body.label.trim();
    if label.is_empty() {
        return Err(DomainError::Validation("显示名不能为空".into()));
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM medal_rarities WHERE value = $1)",
    )
    .bind(&old)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(0));
    }
    // 改键：把用它的勋章一并迁过去（否则那些勋章会掉到"未收录"外观）
    let new_value = body
        .value
        .as_deref()
        .map(clean_rarity_value)
        .filter(|v| !v.is_empty());
    if let Some(ref nv) = new_value {
        if nv != &old {
            let taken: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM medal_rarities WHERE value = $1)",
            )
            .bind(nv)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
            if taken {
                return Err(DomainError::Validation(format!(
                    "稀有度 {nv} 已存在"
                )));
            }
            let mut tx = state
                .repo
                .db
                .begin()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query("UPDATE medals SET rarity = $1 WHERE rarity = $2")
                .bind(nv)
                .bind(&old)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE medal_rarities SET value = $1 WHERE value = $2",
            )
            .bind(nv)
            .bind(&old)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            tx.commit()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    let key = new_value.unwrap_or(old);
    sqlx::query(
        "UPDATE medal_rarities SET label = $2, tone = COALESCE($3, tone), sort = COALESCE($4, sort), \
                updated_at = now() WHERE value = $1",
    )
    .bind(&key)
    .bind(label)
    .bind(body.tone.clone())
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "medal_rarity.update", None)
        .await;
    Ok(ok(serde_json::json!({ "value": key })))
}

#[delete("/admin/medal-rarities/{value}")]
async fn admin_medal_rarity_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MEDAL_MANAGE)
        .await?;
    let value = path.into_inner();
    // 有勋章在用就不许删：否则那些勋章会变成"未收录"外观（宁可让站长先改勋章）
    let used: i64 =
        sqlx::query_scalar("SELECT count(*) FROM medals WHERE rarity = $1")
            .bind(&value)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    if used > 0 {
        return Err(DomainError::Validation(format!(
            "仍有 {used} 枚勋章使用该稀有度，请先把它们改成别的稀有度"
        )));
    }
    let n = sqlx::query("DELETE FROM medal_rarities WHERE value = $1")
        .bind(&value)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit(Some(auth.id), "medal_rarity.del", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": value })))
}
