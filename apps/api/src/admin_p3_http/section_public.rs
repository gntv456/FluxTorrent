//! Section/标签字典公开读 + 分类归属模式与自动过审开关
//! 从 admin_p3_http.rs 按域拆出。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::collections::HashMap;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

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

/// 发布表单/筛选公开读（匿名可读：仅字典名称，与 site-profile 同级）。
///
/// `?category_id=N` 让**分类归属模式真正生效**：`categories.mode_id` 指向的模式的
/// 可见维度由 `mode_kinds(mod_id, kind, visible)` 决定。B2（0195）把口径从
/// 「category_modes 的 7 个固定 show_* 列」改为关联表：
///   · 旧实现按 kind 名 match 那 7 列、`_ => true` ⇒ **站长自建的维度永远不受管辖**，
///     且那 7 个名字（含 audio_codec/standard/processing）本身是影视/音频词表进了 schema；
///   · 关联表下「哪些维度在某模式可见」是**数据**不是列，自建维度同样可被管辖。
/// 缺行语义 = 可见（与旧 `_ => true` 一致，不改行为）。
/// 字典行仍按 `section_dict.mode_id IS NULL OR = 该模式` 过滤（NULL = 全模式可用，
/// 与 /tags-dict 的「全局 + 本分区」口径一致）。不传 category_id 时行为不变。
#[derive(Deserialize)]
struct SectionDictQ {
    #[serde(default)]
    category_id: Option<i32>,
}

#[get("/section-dict")]
async fn section_dict_public(
    _req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SectionDictQ>,
) -> DomainResult<HttpResponse> {
    let mode_id: Option<i32> = match q.category_id {
        Some(cid) => sqlx::query_scalar(
            "SELECT c.mode_id FROM categories c WHERE c.id = $1",
        )
        .bind(cid)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten(),
        None => None,
    };
    // 该模式的维度可见性判定（0195）。两种形态必须区分开，否则「只勾一个维度」
    // 的模式在前台仍是全开（B2 复验 T20 抓到的正是这个）：
    //   · 该模式**有** mode_kinds 行 ⇒ 行集合即白名单（visibible=true 才可见）
    //     —— 这是管理面板「可见维度」勾选框写出来的形态；
    //   · 该模式**没有** mode_kinds 行 ⇒ 老数据/未配置，全部可见
    //     （与旧实现 `_ => true` 行为一致，不制造突变）。
    let (has_rows, visible): (bool, Vec<String>) = match mode_id {
        Some(mid) => {
            let vis: Vec<String> = sqlx::query_scalar(
                "SELECT kind FROM mode_kinds WHERE mode_id = $1 AND visible",
            )
            .bind(mid)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let total: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM mode_kinds WHERE mode_id = $1",
            )
            .bind(mid)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            (total > 0, vis)
        }
        None => (false, Vec::new()),
    };
    let allows = |kind: &str| -> bool {
        if mode_id.is_none() || !has_rows {
            return true;
        }
        visible.iter().any(|v| v == kind)
    };
    let mut out = serde_json::Map::new();
    // 维度清单来自 section_kinds（0085 可配置；0195 起带六类型），预置 9 维已种子化
    let kinds: Vec<SectionKindRow> = sqlx::query_as(
        "SELECT kind, label, sort, field_type, required, multiple, enabled, \
                icon_key, bg_color, 0::bigint AS options \
         FROM section_kinds WHERE enabled ORDER BY sort, kind",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let kinds: Vec<SectionKindRow> =
        kinds.into_iter().filter(|k| allows(&k.kind)).collect();
    for k in &kinds {
        let rows: Vec<(i64, String, String, i32, Option<i32>)> =
            sqlx::query_as(
                "SELECT id, kind, name, sort, mode_id FROM section_dict \
                 WHERE kind = $1 AND ($2::int IS NULL OR mode_id IS NULL \
                 OR mode_id = $2) ORDER BY sort, id",
            )
            .bind(&k.kind)
            .bind(mode_id)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let rows: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|(id, kind, name, sort, mid)| {
                serde_json::json!({"id": id, "kind": kind, "name": name,
                                   "sort": sort, "mode_id": mid})
            })
            .collect();
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
        r#"SELECT m.id, m.name, m.show_source, m.show_medium, m.show_codec,
                  m.show_audio_codec, m.show_standard, m.show_processing,
                  m.show_team,
                  (SELECT count(*) FROM categories c
                    WHERE c.mode_id = m.id)::bigint AS categories,
                  (SELECT COALESCE(json_agg(mk.kind ORDER BY mk.kind),
                                   '[]'::json)
                     FROM mode_kinds mk
                    WHERE mk.mode_id = m.id AND mk.visible)
                      AS visible_kinds,
                  (SELECT COALESCE(json_agg(mk.kind ORDER BY mk.kind),
                                   '[]'::json)
                     FROM mode_kinds mk
                    WHERE mk.mode_id = m.id AND NOT mk.visible)
                      AS hidden_kinds
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
    let mode: Option<i32> =
        web::Query::<HashMap<String, String>>::from_query(req.query_string())
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
    let tuples: Vec<(i32, String, String)> = rows
        .iter()
        .map(|r| (r.0, r.1.clone(), r.2.clone()))
        .collect();
    let groups: Vec<&str> = rows.iter().map(|r| r.3.as_str()).collect();
    Ok(ok(serde_json::json!({ "tags": tuples, "groups": groups })))
}

/// 分类归属模式 + 自动过审开关
#[derive(Deserialize)]
struct CategoryFlagsReq {
    /// 缺省 = 不改归属；`0` = 显式清除归属（回到「全模式可用」）；其余 = 改归属。
    /// 早先实现是裸 `SET mode_id = $2`，前端只勾「自动过审」不带 mode_id 时会把
    /// 归属写成 NULL（等于静默改分类归属）。
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
        "UPDATE categories SET \
           mode_id = CASE WHEN $2::int IS NULL THEN mode_id \
                          WHEN $2::int = 0 THEN NULL \
                          ELSE $2::int END, \
           auto_approve = COALESCE($3, auto_approve) \
         WHERE id = $1",
    )
    .bind(id)
    .bind(body.mode_id)
    .bind(body.auto_approve)
    .execute(&state.repo.db)
    .await
    .map_err(|e| crate::errors::db_to_domain(e, "分类归属模式"))?
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
