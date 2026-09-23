//! Section/标签字典公开读 + 分类归属模式与自动过审开关
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::section_dict::section_dict_rows;
use super::section_kinds::SectionKindRow;
use super::sections::SectionModeRow;
use super::staff;

/// 维度标识格式：小写字母开头，[a-z0-9_]，≤32 字符
pub(super) fn regex_check_kind(kind: &str) -> bool {
    let bytes = kind.as_bytes();
    bytes.len() <= 32
        && !bytes.is_empty()
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}

/// 发布表单/筛选公开读（匿名可读：仅字典名称，与 site-profile 同级）
#[get("/section-dict")]
async fn section_dict_public(
    _req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let mut out = serde_json::Map::new();
    // 维度清单来自 section_kinds（0085 可配置），预置 9 维已种子化
    let kinds: Vec<SectionKindRow> = sqlx::query_as(
        "SELECT kind, label, sort FROM section_kinds ORDER BY sort, kind",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for k in &kinds {
        let rows = section_dict_rows(&state.repo.db, Some(&k.kind)).await?;
        out.insert(
            k.kind.clone(),
            serde_json::to_value(rows).unwrap_or_default(),
        );
    }
    out.insert(
        "kinds".into(),
        serde_json::to_value(&kinds).unwrap_or_default(),
    );
    let modes: Vec<SectionModeRow> = sqlx::query_as(
        r#"SELECT m.id, m.name, m.show_source, m.show_medium, m.show_codec, m.show_audio_codec,
                  m.show_standard, m.show_processing, m.show_team,
                  (SELECT count(*) FROM categories c WHERE c.mode_id = m.id)::bigint AS categories
           FROM category_modes m ORDER BY m.id"#,
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    out.insert(
        "modes".into(),
        serde_json::to_value(modes).unwrap_or_default(),
    );
    Ok(ok(serde_json::Value::Object(out)))
}

/// 发布表单标签公开读（匿名可读：启用中的种子域标签，0138 scope=torrent；论坛域走 /forums/tags）。
/// 0160 P2：带 tag_group（前端按组分区）+ mode 过滤（`?mode=N`：全局标签 + 该分区专属，
/// NP v1.8 `mode IN (0, searchBoxId)` 口径；缺省不过滤）。返回保持 [id,name,kind] 元组
/// 兼容旧消费方，组别经平行数组 `groups` 下发（元组形态不破坏）。
#[get("/tags-dict")]
async fn tags_dict_public(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let mode: Option<i32> = web::Query::<std::collections::HashMap<String, String>>::from_query(
        req.query_string(),
    )
    .ok()
    .and_then(|m| m.get("mode").and_then(|v| v.parse().ok()));
    let rows: Vec<(i32, String, String, String)> = sqlx::query_as(
        "SELECT id, name, kind, tag_group FROM tag_dict \
         WHERE COALESCE(enabled, TRUE) AND scope = 'torrent' \
         AND ($1::int IS NULL OR mode_id IS NULL OR mode_id = $1) \
         ORDER BY sort DESC, id",
    )
    .bind(mode)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 元组主数组（旧形态）+ groups 平行数组（下标对齐）：老客户端读前三项无感
    let tuples: Vec<(i32, String, String)> =
        rows.iter().map(|r| (r.0, r.1.clone(), r.2.clone())).collect();
    let groups: Vec<&str> = rows.iter().map(|r| r.3.as_str()).collect();
    Ok(ok(serde_json::json!({ "tags": tuples, "groups": groups })))
}

/// 分类归属模式 + 自动过审开关
#[derive(Deserialize)]
struct CategoryFlagsReq {
    #[serde(default)]
    mode_id: Option<i32>,
    #[serde(default)]
    auto_approve: Option<bool>,
}

#[put("/admin/categories/{id}/flags")]
async fn category_flags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<CategoryFlagsReq>,
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
        "UPDATE categories SET mode_id = $2, \
         auto_approve = COALESCE($3, auto_approve) WHERE id = $1",
    )
    .bind(id)
    .bind(body.mode_id)
    .bind(body.auto_approve)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(id as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "category.flags", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
