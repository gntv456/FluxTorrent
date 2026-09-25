//! P3-12 Section 多维体系：分类模式 CRUD + is_custom_kind 维度判定。
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::staff;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

// ============ P3-12 Section 多维体系 ============

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct SectionModeRow {
    id: i32,
    name: String,
    // ⚠ 以下 7 列是 0063 的旧口径，**本批保留**（站长拍板：保留一个发布周期，
    // 回滚容易）。B2 起读取侧改用 `mode_kinds` 关联表，见 visible_kinds；
    // 这 7 个字段仅为兼容存量消费点继续下发，不再是权威来源。
    show_source: bool,
    show_medium: bool,
    show_codec: bool,
    show_audio_codec: bool,
    show_standard: bool,
    show_processing: bool,
    show_team: bool,
    categories: i64,
    /// B2（0195）：该模式下**可见**的维度清单（来自 mode_kinds）。
    /// 无行的维度 = 可见（与旧 `_ => true` 一致），故这里是「被显式关闭」的反面。
    visible_kinds: Option<serde_json::Value>,
    /// B2：该模式下**被显式关闭**的维度清单（管理面板直接编辑这个集合）。
    hidden_kinds: Option<serde_json::Value>,
}

#[get("/admin/section-modes")]
async fn section_modes_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<SectionModeRow> = sqlx::query_as(
        r#"SELECT m.id, m.name, m.show_source, m.show_medium, m.show_codec, m.show_audio_codec,
                  m.show_standard, m.show_processing, m.show_team,
                  (SELECT count(*) FROM categories c WHERE c.mode_id = m.id)::bigint AS categories,
                  (SELECT COALESCE(json_agg(mk.kind ORDER BY mk.kind), '[]'::json)
                     FROM mode_kinds mk WHERE mk.mode_id = m.id AND mk.visible)
                      AS visible_kinds,
                  (SELECT COALESCE(json_agg(mk.kind ORDER BY mk.kind), '[]'::json)
                     FROM mode_kinds mk WHERE mk.mode_id = m.id AND NOT mk.visible)
                      AS hidden_kinds
           FROM category_modes m ORDER BY m.id"#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::to_value(rows).unwrap_or_default()))
}

#[derive(Deserialize)]
struct SectionModeReq {
    name: String,
    #[serde(default)]
    show_source: Option<bool>,
    #[serde(default)]
    show_medium: Option<bool>,
    #[serde(default)]
    show_codec: Option<bool>,
    #[serde(default)]
    show_audio_codec: Option<bool>,
    #[serde(default)]
    show_standard: Option<bool>,
    #[serde(default)]
    show_processing: Option<bool>,
    #[serde(default)]
    show_team: Option<bool>,
    /// B2（0195）：该模式的**可见维度**整组提交（`mode_kinds` 唯一权威入口）。
    /// `Some([...])` = 用这一组替换该模式全部 mode_kinds 行（不在组内的维度
    /// 在改分类重取 /section-dict 时消失）；`None` = 不动（仅改旧 7 列）。
    /// 自建维度只能从这里纳入管辖——旧 7 列写不下它们。
    #[serde(default)]
    visible_kinds: Option<Vec<String>>,
}

#[post("/admin/section-modes")]
async fn section_mode_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SectionModeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("模式名不能为空".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO category_modes (name, show_source, show_medium, show_codec, show_audio_codec, show_standard, show_processing, show_team) \
         VALUES ($1, COALESCE($2, TRUE), COALESCE($3, TRUE), COALESCE($4, TRUE), COALESCE($5, TRUE), COALESCE($6, TRUE), COALESCE($7, TRUE), COALESCE($8, TRUE)) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.show_source)
    .bind(body.show_medium)
    .bind(body.show_codec)
    .bind(body.show_audio_codec)
    .bind(body.show_standard)
    .bind(body.show_processing)
    .bind(body.show_team)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "section_mode.add", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/section-modes/{id}")]
async fn section_mode_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<SectionModeReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    let n = sqlx::query(
        "UPDATE category_modes SET name = $2, show_source = COALESCE($3, show_source), \
           show_medium = COALESCE($4, show_medium), show_codec = COALESCE($5, show_codec), \
           show_audio_codec = COALESCE($6, show_audio_codec), show_standard = COALESCE($7, show_standard), \
           show_processing = COALESCE($8, show_processing), show_team = COALESCE($9, show_team) WHERE id = $1",
    )
    .bind(id)
    .bind(body.name.trim())
    .bind(body.show_source)
    .bind(body.show_medium)
    .bind(body.show_codec)
    .bind(body.show_audio_codec)
    .bind(body.show_standard)
    .bind(body.show_processing)
    .bind(body.show_team)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    // B2（0195）：mode_kinds 整组重建（自建维度的唯一管辖入口）。
    // 校验：每个 kind 必须存在于 section_kinds（否则挂了个不存在的维度，
    // 前台白拿不出字典项，属于静默错配）。
    if let Some(kinds) = &body.visible_kinds {
        let mut seen: Vec<String> = Vec::new();
        for k in kinds {
            let k = k.trim();
            if k.is_empty() || seen.iter().any(|s| s == k) {
                continue;
            }
            if !is_custom_kind(&state.repo.db, k).await {
                return Err(DomainError::Validation(format!(
                    "未知维度 {k}"
                )));
            }
            seen.push(k.to_string());
        }
        let mut tx = state
            .repo
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("DELETE FROM mode_kinds WHERE mode_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        for k in &seen {
            sqlx::query(
                "INSERT INTO mode_kinds (mode_id, kind, visible) \
                 VALUES ($1, $2, true)",
            )
            .bind(id)
            .bind(k)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        // 同步回旧 7 列：本批「保留一个周期」，两边不能说法不一（回滚也不炸）
        sync_legacy_mode_columns(&state.repo.db, id, &seen).await?;
    }
    state
        .repo
        .audit(Some(auth.id), "section_mode.update", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 把 `mode_kinds` 的可见集合回写进 `category_modes` 的旧 7 个 show_* 列。
/// 旧七维与 kind 名的固定映射，仅用于保留期兼容，不是权威来源。
const LEGACY_MODE_KEYS: [(&str, &str); 7] = [
    ("source", "show_source"),
    ("media", "show_medium"),
    ("codec", "show_codec"),
    ("audio_codec", "show_audio_codec"),
    ("standard", "show_standard"),
    ("processing", "show_processing"),
    ("team", "show_team"),
];

async fn sync_legacy_mode_columns(
    db: &sqlx::PgPool,
    mode_id: i32,
    visible: &[String],
) -> DomainResult<()> {
    let mut sets: Vec<String> = Vec::new();
    for (kind, col) in LEGACY_MODE_KEYS {
        sets.push(format!(
            "{col} = {}",
            if visible.iter().any(|v| v == kind) {
                "TRUE"
            } else {
                "FALSE"
            }
        ));
    }
    sqlx::query(&format!(
        "UPDATE category_modes SET {} WHERE id = $1",
        sets.join(", ")
    ))
    .bind(mode_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

#[delete("/admin/section-modes/{id}")]
async fn section_mode_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    if id == 1 {
        return Err(DomainError::Validation("默认模式不可删除".into()));
    }
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM categories WHERE mode_id = $1",
    )
    .bind(id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if used > 0 {
        sqlx::query("UPDATE categories SET mode_id = 1 WHERE mode_id = $1")
            .bind(id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    let n = sqlx::query("DELETE FROM category_modes WHERE id = $1")
        .bind(id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "section_mode.del", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

/// 维度存在性判定（0085/0087）：kind 在 section_kinds 中即可用。
/// 0087 起 media/grades/editions 字典行已迁入 section_dict，九维全走统一通道，
/// legacy 实体表仅作历史口径存档（介质列保留兼容老数据）。
pub(crate) async fn is_custom_kind(db: &sqlx::PgPool, kind: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM section_kinds WHERE kind = $1)",
    )
    .bind(kind)
    .fetch_one(db)
    .await
    .unwrap_or(false)
}
