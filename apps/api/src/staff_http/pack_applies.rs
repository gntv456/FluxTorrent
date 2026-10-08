//! 站型包 apply 台账（G21）：回看（diff/计数/时间/人）与回滚。
//!
//! 回滚口径沿用 0167 内容包先例——**把应用前的快照当一次 apply 再执行一遍**，
//! 而不是维护逆操作：分类/模块/维度/标签/等级/经济/元数据/术语/字幕口径全在
//! 快照里，重放即还原。只允许回滚「最近一次未回滚的应用」，避免把站点拧成
//! 「A 的分类 + B 的模块」的混合态。

use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Serialize;
use serde_json::json;

use super::sitetype::SiteTypePack;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Serialize, sqlx::FromRow)]
struct ApplyRow {
    id: i64,
    pack_code: String,
    pack_name: String,
    mode: String,
    changes: serde_json::Value,
    counts: serde_json::Value,
    applied_at: String,
    actor: Option<String>,
    rolled_back_at: Option<String>,
}

/// 应用台账（最近 30 条；不含 snapshot——体积大且只有回滚用）
#[get("/admin/site-type-packs/applies")]
pub async fn site_pack_applies(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let rows: Vec<ApplyRow> = sqlx::query_as(
        "SELECT a.id, a.pack_code, a.pack_name, a.mode, a.changes, a.counts, \
         to_char(a.applied_at, 'YYYY-MM-DD HH24:MI') AS applied_at, \
         u.username::text AS actor, \
         to_char(a.rolled_back_at, 'YYYY-MM-DD HH24:MI') AS rolled_back_at \
         FROM site_pack_applies a LEFT JOIN users u ON u.id = a.applied_by \
         ORDER BY a.id DESC LIMIT 30",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 快照 → 可执行的临时包（回滚专用；不在 site_type_packs 表里，故 extras
/// 走 apply_pack_extras_json 载荷版而不是按 code 查表）
fn snapshot_to_pack(
    snapshot: &serde_json::Value,
    name: &str,
) -> DomainResult<SiteTypePack> {
    let seg = |k: &str| -> Option<serde_json::Value> {
        match snapshot.get(k) {
            Some(v) if !v.is_null() => Some(v.clone()),
            _ => None,
        }
    };
    let site = snapshot.get("site").cloned().unwrap_or_default();
    let text = |k: &str| -> Option<String> {
        site.get(k).and_then(|v| v.as_str()).map(|s| s.to_string())
    };
    let code = text("site_type").unwrap_or_default();
    if code.is_empty() {
        return Err(DomainError::Validation("快照缺少站型，无法回滚".into()));
    }
    Ok(SiteTypePack {
        code,
        name: name.to_string(),
        description: None,
        brand: text("site_name").unwrap_or_default(),
        categories: seg("categories").unwrap_or_else(|| json!([])),
        modules: seg("modules").unwrap_or_else(|| json!({})),
        sort: 0,
        sections: seg("sections"),
        tags: seg("tags"),
        tagline: text("site_tagline").unwrap_or_default(),
        subtitle_kind: text("subtitle_kind"),
        terms: seg("terms"),
    })
}

/// 回滚到某次应用之前（只允许最近一次未回滚的应用）
#[post("/admin/site-type-packs/applies/{id}/rollback")]
pub async fn site_pack_rollback(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let id = *path;
    let row: Option<(String, serde_json::Value, Option<String>, bool)> =
        sqlx::query_as(
            "SELECT pack_name, snapshot, \
             to_char(rolled_back_at, 'YYYY-MM-DD HH24:MI'), \
             COALESCE(id = (SELECT max(id) FROM site_pack_applies \
             WHERE rolled_back_at IS NULL), false) \
             FROM site_pack_applies WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((pack_name, snapshot, rolled, is_latest)) = row else {
        return Err(DomainError::Validation("应用记录不存在".into()));
    };
    if rolled.is_some() {
        return Err(DomainError::Validation("该次应用已回滚过".into()));
    }
    if !is_latest {
        return Err(DomainError::Validation(
            "只能回滚最近一次应用：其后还有更新的记录，请先回滚它".into(),
        ));
    }
    let pack = snapshot_to_pack(&snapshot, &pack_name)?;
    // 1) 快照当一次 apply 执行（restore：图标以快照为准；非 replace 语义，
    //    有种子的站点也能回滚）
    let (added, _extras) =
        super::pack_core::apply_pack_full(&state.repo.db, &pack, "restore")
            .await?;
    // 2) 快照段补齐：等级/经济/元数据按快照覆盖（apply_pack_full 内建 extras 是
    //    按 code 查表的，回滚包不在表里）；标语精确还原（apply 的置空回落不适用）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("SELECT * FROM apply_pack_extras_json($1::jsonb)")
        .bind(snapshot.to_string())
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO site_settings (name, value) VALUES ('site_tagline', $1) \
         ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
         updated_at = now()",
    )
    .bind(&pack.tagline)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 分类色（0183）不在 upsert 列里：replace 整表重建会把它抹掉，按快照补回
    sqlx::query(
        "UPDATE categories c SET bg_color = e->>'bg_color' FROM \
         jsonb_array_elements($1::jsonb) e WHERE (e->>'id')::int = c.id",
    )
    .bind(pack.categories.to_string())
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 3) 术语段（自建事务；字幕口径已在 apply_pack_full 内按快照应用）
    let terms_applied =
        super::pack_terms::apply_pack_terms(&state.repo.db, &pack).await?;
    crate::cfgver::bump(state.get_ref(), "terms").await;
    // 4) 清理本次 apply 新建、快照里没有的分类：被引用（种子/促销/愿望单等外键
    //    拒绝）或有子级的一律保留——宁残留不破坏。
    //    0317 起优先按 key 对（快照分类带 key；id 仅对老快照回落）
    let stale: Vec<i32> = sqlx::query_scalar(
        "SELECT c.id FROM categories c WHERE NOT EXISTS (SELECT 1 FROM \
         jsonb_array_elements($1::jsonb) e WHERE \
           (e->>'key' IS NOT NULL AND e->>'key' = c.key) \
           OR (e->>'key' IS NULL AND (e->>'id')::int = c.id))",
    )
    .bind(pack.categories.to_string())
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    for cid in stale {
        let _ = sqlx::query(
            "DELETE FROM categories c WHERE c.id = $1 AND NOT EXISTS \
             (SELECT 1 FROM categories k WHERE k.parent_id = c.id)",
        )
        .bind(cid)
        .execute(&state.repo.db)
        .await;
    }
    // 5) 标记已回滚（保留快照便于审计）+ 审计 + 模块开关缓存失效
    sqlx::query(
        "UPDATE site_pack_applies SET rolled_back_at = now(), \
         rolled_back_by = $2 WHERE id = $1",
    )
    .bind(id)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.module_flags.invalidate().await;
    crate::cfgver::bump(state.get_ref(), "modules").await;
    state
        .repo
        .audit(Some(auth.id), "site_type_pack_rollback", Some(id))
        .await;
    Ok(ok(json!({ "rolled_back": id, "site_type": pack.code,
        "categories": added, "terms_applied": terms_applied })))
}
