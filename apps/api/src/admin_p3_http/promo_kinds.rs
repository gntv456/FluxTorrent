//! 促销档位注册表 CRUD（0213）：服务「不偏向任何 PT 类型」定位——站方可自定义
//! 用户自购的推广档位（类型 + 时长 + 价格 + 落地效果）。
//! 本文件承载「档位类型」；价目档在 promo_tiers.rs。权限 SETTINGS_MANAGE。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;
/// 落地效果白名单：sticky1/sticky2 写 pos_state，其余写 promotions(kind=effect)。
/// 与 worker billing_multipliers 的 match 分支严格一致（改这里必须同步 worker）。
pub(super) const EFFECTS: &[&str] = &[
    "sticky1", "sticky2", "free", "x2", "x2free", "half", "x2half", "p30",
];

#[derive(sqlx::FromRow, serde::Serialize)]
struct KindAdminRow {
    kind: String,
    label_zh: String,
    label_en: String,
    effect: String,
    i18n_key: Option<String>,
    sort_order: i32,
    enabled: bool,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct TierAdminRow {
    pub(super) id: i64,
    pub(super) kind: String,
    pub(super) hours: i32,
    pub(super) price: i64,
    pub(super) enabled: bool,
}

const KIND_SELECT: &str =
    "SELECT kind, label_zh, label_en, effect, i18n_key, sort_order, \
     enabled FROM promo_kinds ORDER BY sort_order, kind";

const TIER_SELECT: &str = "SELECT id, kind, hours, price, enabled \
     FROM promo_kind_tiers ORDER BY kind, hours";

/// 档位 + 价目一次取全（前端两张表渲染）
#[get("/admin/promo-kinds")]
pub(super) async fn promo_kinds_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let kinds: Vec<KindAdminRow> = sqlx::query_as(KIND_SELECT)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let tiers: Vec<TierAdminRow> = sqlx::query_as(TIER_SELECT)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "kinds": kinds,
        "tiers": tiers,
        "effects": EFFECTS,
    })))
}

#[derive(Deserialize)]
pub(super) struct KindReq {
    pub(super) kind: String,
    pub(super) label_zh: String,
    #[serde(default)]
    pub(super) label_en: Option<String>,
    pub(super) effect: String,
    #[serde(default)]
    pub(super) i18n_key: Option<String>,
    #[serde(default)]
    pub(super) sort_order: Option<i32>,
    #[serde(default)]
    pub(super) enabled: Option<bool>,
}

fn validate_new_kind(body: &KindReq) -> DomainResult<()> {
    let k = body.kind.trim();
    if k.is_empty() || k.len() > 40 {
        return Err(DomainError::Validation("档位标识长度 1-40".into()));
    }
    if !k
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(DomainError::Validation(
            "档位标识仅允许字母/数字/下划线/连字符".into(),
        ));
    }
    validate_label_effect(body.label_zh.trim(), &body.effect, true)
}

fn validate_label_effect(
    label: &str,
    effect: &str,
    required: bool,
) -> DomainResult<()> {
    if required && (label.is_empty() || label.len() > 60) {
        return Err(DomainError::Validation("显示名长度 1-60".into()));
    }
    if !effect.is_empty() && !EFFECTS.contains(&effect) {
        return Err(DomainError::Validation(format!(
            "落地效果需为以下之一：{}",
            EFFECTS.join(" / ")
        )));
    }
    Ok(())
}

#[post("/admin/promo-kinds")]
pub(super) async fn promo_kind_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<KindReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    validate_new_kind(&body)?;
    let kind = body.kind.trim();
    // 重名映射 400（别让 UNIQUE 冲突变 500）
    let dup: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM promo_kinds WHERE kind = $1)",
    )
    .bind(kind)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if dup {
        return Err(DomainError::Validation("档位标识已存在".into()));
    }
    sqlx::query(
        "INSERT INTO promo_kinds \
         (kind, label_zh, label_en, effect, i18n_key, sort_order, enabled) \
         VALUES ($1, $2, COALESCE($3, ''), $4, $5, COALESCE($6, 100), \
         COALESCE($7, TRUE))",
    )
    .bind(kind)
    .bind(body.label_zh.trim())
    .bind(body.label_en.clone())
    .bind(&body.effect)
    .bind(body.i18n_key.clone())
    .bind(body.sort_order)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "promo_kind.add", None)
        .await;
    Ok(ok(serde_json::json!({ "kind": kind })))
}

/// 更新档位（标识不可改——已被 promo_purchases 引用）
#[put("/admin/promo-kinds/{kind}")]
pub(super) async fn promo_kind_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
    body: web::Json<KindReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let kind = path.into_inner();
    validate_label_effect(body.label_zh.trim(), &body.effect, true)?;
    let n = sqlx::query(
        "UPDATE promo_kinds SET label_zh = $2, \
            label_en = COALESCE($3, label_en), \
            effect = COALESCE($4, effect), \
            i18n_key = COALESCE($5, i18n_key), \
            sort_order = COALESCE($6, sort_order), \
            enabled = COALESCE($7, enabled) \
         WHERE kind = $1",
    )
    .bind(&kind)
    .bind(body.label_zh.trim())
    .bind(body.label_en.clone())
    .bind(Some(&body.effect).filter(|e| !e.is_empty()))
    .bind(body.i18n_key.clone())
    .bind(body.sort_order)
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_kind.update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 删除档位：有购买记录的只允许停用（保留历史，同 shop_items 口径），
/// 无记录的连同价目一起删（tiers 走 ON DELETE CASCADE）。
#[delete("/admin/promo-kinds/{kind}")]
pub(super) async fn promo_kind_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let kind = path.into_inner();
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM promo_purchases WHERE kind = $1",
    )
    .bind(&kind)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if used > 0 {
        let n = sqlx::query(
            "UPDATE promo_kinds SET enabled = FALSE WHERE kind = $1",
        )
        .bind(&kind)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        if n == 0 {
            return Err(DomainError::NotFound(0));
        }
        state
            .repo
            .audit(Some(auth.id), "promo_kind.disable", None)
            .await;
        return Ok(ok(serde_json::json!({ "disabled": true })));
    }
    let n = sqlx::query("DELETE FROM promo_kinds WHERE kind = $1")
        .bind(&kind)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_kind.delete", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": kind })))
}
