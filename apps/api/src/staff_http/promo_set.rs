//! 促销设置应用。
//! 从 staff_http.rs 按域拆出。

use super::promo::promo_parse;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{delete, get, post, put, web, HttpRequest, Responder};
use serde::Deserialize;

#[post("/admin/freeleech")]
pub async fn freeleech_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FreeleechBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let (kind, scope, starts_at, ends_at) = promo_parse(
        &body.kind,
        body.scope.as_deref(),
        body.hours,
        &body.starts_at,
        &body.ends_at,
    )?;
    let kind = kind.as_str();
    let scope = scope.as_str();
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if scope == "category" {
        let Some(cid) = body.category_id else {
            return Err(DomainError::Validation("分类促销需指定分类".into()));
        };
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
                .bind(cid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("分类不存在".into()));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO promotions (scope, category_id, kind, starts_at, ends_at, source, created_by) \
             VALUES ('category', $1, $2::promotion_kind_enum, $3, $4, 'manual', $5) RETURNING id",
        ).bind(cid).bind(kind).bind(starts_at).bind(ends_at).bind(auth.id)
        .fetch_one(&mut *tx).await.map_err(|e| DomainError::Internal(e.into()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        state.repo.audit(Some(auth.id), "promo_set", None).await;
        return Ok(ok(
            serde_json::json!({ "id": id, "kind": kind, "scope": scope, "category_id": cid, "hours": body.hours }),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO promotions (scope, kind, starts_at, ends_at, source, created_by) \
         VALUES ($1::promotion_scope, $2::promotion_kind_enum, $3, $4, 'manual', $5) RETURNING id",
    )
    .bind(scope)
    .bind(kind)
    .bind(starts_at)
    .bind(ends_at)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "promo_set", None).await;
    Ok(ok(
        serde_json::json!({ "id": id, "kind": kind, "scope": scope, "hours": body.hours }),
    ))
}

#[delete("/admin/freeleech")]
pub async fn freeleech_clear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    // 清除全部进行中的手动站点级促销（全站/官种/非官种/分类）
    let n = sqlx::query("DELETE FROM promotions WHERE scope IN ('global','official','non_official','category') AND source='manual' AND ends_at > now()")
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state
        .repo
        .audit(Some(auth.id), "freeleech_clear", None)
        .await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

/// 编辑单条促销（类型/范围/起止时间均可改）
#[put("/admin/freeleech/{id}")]
pub async fn freeleech_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<FreeleechBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let pid = path.into_inner();
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM promotions WHERE id = $1 AND source = 'manual'",
    )
    .bind(pid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::Validation("促销不存在或非手动创建".into()));
    }
    let (kind, scope, starts_at, ends_at) = promo_parse(
        &body.kind,
        body.scope.as_deref(),
        body.hours,
        &body.starts_at,
        &body.ends_at,
    )?;
    if scope == "category" {
        let Some(cid) = body.category_id else {
            return Err(DomainError::Validation("分类促销需指定分类".into()));
        };
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
                .bind(cid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("分类不存在".into()));
        }
        sqlx::query("UPDATE promotions SET scope='category', category_id=$1, kind=$2::promotion_kind_enum, starts_at=$3, ends_at=$4 WHERE id=$5")
            .bind(cid).bind(&kind).bind(starts_at).bind(ends_at).bind(pid)
            .execute(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("UPDATE promotions SET scope=$1::promotion_scope, category_id=NULL, kind=$2::promotion_kind_enum, starts_at=$3, ends_at=$4 WHERE id=$5")
            .bind(&scope).bind(&kind).bind(starts_at).bind(ends_at).bind(pid)
            .execute(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "promo_update", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "updated": pid })))
}

/// 删除单条促销（不影响其他并存促销）
#[delete("/admin/freeleech/{id}")]
pub async fn freeleech_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let pid = path.into_inner();
    let n = sqlx::query(
        "DELETE FROM promotions WHERE id = $1 AND source = 'manual'",
    )
    .bind(pid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("促销不存在或非手动创建".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_delete", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": pid })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct SitePromoRow {
    id: i64,
    scope: String,
    kind: String,
    category_id: Option<i32>,
    category_name: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/freeleech")]
pub async fn freeleech_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_VIEW,
    )
    .await?;
    let rows: Vec<SitePromoRow> = sqlx::query_as(
        "SELECT p.id, p.scope::text AS scope, p.kind::text AS kind, p.category_id, c.name AS category_name, p.starts_at, p.ends_at \
         FROM promotions p LEFT JOIN categories c ON c.id = p.category_id \
         WHERE p.scope IN ('global','official','non_official','category') AND p.source='manual' AND p.ends_at > now() \
         ORDER BY p.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 种子促销（原"免费下载"，freeleech.php 升级口径）：
/// scope = global 全站 | official 官种 | non_official 非官种 | category 某分类
/// 支持自定义起止时间（可预约：到达开始时间自动生效）；同范围可并存多个促销（计费取最强档）
#[derive(Deserialize)]
pub(super) struct FreeleechBody {
    pub(super) kind: String, // free / x2 / x2free / half / x2half / p30
    pub(super) hours: i32,   // 结束时间未提供时用（自开始时间起算）
    #[serde(default)]
    pub(super) scope: Option<String>,
    #[serde(default)]
    pub(super) category_id: Option<i32>,
    #[serde(default)]
    pub(super) starts_at: Option<String>, // RFC3339，缺省=now
    #[serde(default)]
    pub(super) ends_at: Option<String>, // RFC3339，缺省=starts_at+hours
}
