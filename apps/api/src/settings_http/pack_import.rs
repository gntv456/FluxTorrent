//! 内容包导入与回滚的 HTTP 编排（M1 核心）：快照 → 纯覆盖落库 → 登记 → 审计。
//! 从 packs.rs 按域拆出（300 行门禁）。格式与导出在 pack_format.rs，
//! 落库原语（快照/守卫/apply）在 pack_store.rs。
//!
//! 路径纪律：
//! - 导入（confirm=true）在**单事务**内：拍快照 → 防悬挂检查 → 落库 → 写
//!   content_packs（同 pack_id 升级覆盖快照）→ 审计 → 提交；失败整体回退；
//! - 回滚 = 读 content_packs.snapshot，包装成一次「逆向导入」走同一条纯覆盖
//!   路径重放（A2：回滚不依赖逆向 diff，永远可用），回滚后登记行删除；
//! - 预检（confirm=false）只读：重名分类悬挂检查 + theme 白名单/校验 + 差异计数。

use actix_web::{web, HttpResponse};
use serde_json::Value;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::engine::validate_field;
use super::groups::invalidate_cache;
use super::meta::MetaRow;
use super::pack_format::{
    bad, parse_pack, parse_taxonomy, PackHead, THEME_KEYS,
};
use super::pack_store::{
    apply_assets, apply_taxonomy, apply_taxonomy_guarded,
    apply_theme_import, apply_theme_rollback, referenced_categories,
    snapshot_assets, snapshot_taxonomy, snapshot_theme,
    snapshot_to_payload,
};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

// ============ 导入（packs.rs 调用） ============

pub(super) async fn import(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    pack: &Value,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let head = parse_pack(pack)?;
    match head.kind.as_str() {
        "taxonomy" => import_taxonomy(state, actor, &head, confirm).await,
        "rules" => import_rules(state, actor, &head, confirm).await,
        "assets" => import_assets(state, actor, &head, confirm).await,
        "embed" => import_embed(state, actor, &head, confirm).await,
        _ => import_theme(state, actor, &head, confirm).await,
    }
}

/// embed 包导入（0190）：payload.rules = [video_embed_rules 行数组]。
/// 白名单只有 video_embed_rules 一张表；快照重放（同 assets 口径）。
/// builtin 行不受包影响（包只覆盖非 builtin 行；builtin 种子跟版本走）。
async fn import_embed(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let Some(rows) = head.payload.get("rules").and_then(Value::as_array)
    else {
        return Err(bad("embed 包缺少 payload.rules"));
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(internal)?;
    if !confirm {
        tx.rollback().await.ok();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": "embed",
            "pack_id": head.pack_id,
            "will_change": rows.len(),
        })));
    }
    // 快照（回滚 = 逆向重放，同 assets 口径）
    let snap: Vec<(i64, String, String, String, String, String)> =
        sqlx::query_as(
            "SELECT id, provider, url_pattern, embed_template, \
             embed_origin, render_kind FROM video_embed_rules \
             WHERE NOT builtin ORDER BY id",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(internal)?;
    let snapshot = serde_json::json!({
        "rules": snap.iter().map(|r| serde_json::json!({
            "id": r.0, "provider": r.1, "url_pattern": r.2,
            "embed_template": r.3, "embed_origin": r.4,
            "render_kind": r.5,
        })).collect::<Vec<_>>()
    });
    // 纯覆盖：删包外非 builtin 行 → 逐行 upsert
    let keep: Vec<i64> = rows
        .iter()
        .filter_map(|r| r.get("id").and_then(Value::as_i64))
        .collect();
    sqlx::query(
        "DELETE FROM video_embed_rules WHERE NOT builtin \
         AND NOT (id = ANY($1))",
    )
    .bind(&keep)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    let mut applied = 0usize;
    for r in rows {
        let s = |k: &str| {
            r.get(k).and_then(Value::as_str).unwrap_or_default()
        };
        let id = r.get("id").and_then(Value::as_i64).unwrap_or(0);
        if id <= 0 {
            continue;
        }
        sqlx::query(
            "INSERT INTO video_embed_rules (id, provider, name_zh, \
             url_pattern, embed_template, embed_origin, render_kind, \
             aspect, extra_params, builtin, enabled, sort) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,FALSE,$10,$11) \
             ON CONFLICT (id) DO UPDATE SET \
             provider=EXCLUDED.provider, name_zh=EXCLUDED.name_zh, \
             url_pattern=EXCLUDED.url_pattern, \
             embed_template=EXCLUDED.embed_template, \
             embed_origin=EXCLUDED.embed_origin, \
             render_kind=EXCLUDED.render_kind, aspect=EXCLUDED.aspect, \
             extra_params=EXCLUDED.extra_params, \
             enabled=EXCLUDED.enabled, sort=EXCLUDED.sort",
        )
        .bind(id)
        .bind(s("provider"))
        .bind(s("name_zh"))
        .bind(s("url_pattern"))
        .bind(s("embed_template"))
        .bind(s("embed_origin"))
        .bind(s("render_kind"))
        .bind(s("aspect"))
        .bind(r.get("extra_params").cloned().unwrap_or(Value::Null))
        .bind(r.get("enabled").and_then(Value::as_bool).unwrap_or(true))
        .bind(r.get("sort").and_then(Value::as_i64).unwrap_or(100) as i32)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        applied += 1;
    }
    // 重放需要 sequence 对齐（显式 id 插入不推进 bigserial）
    sqlx::query(
        "SELECT setval('video_embed_rules_id_seq', \
         GREATEST((SELECT COALESCE(max(id),1) FROM video_embed_rules), 1))",
    )
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    upsert_pack_row(&mut tx, actor, head, snapshot).await?;
    audit(
        &mut tx,
        *actor,
        "content_pack:import",
        &serde_json::json!({
            "pack_id": head.pack_id, "kind": head.kind,
            "name": head.name, "version": head.version,
        }),
    )
    .await;
    tx.commit().await.map_err(internal)?;
    invalidate_cache(state, "module").await;
    // embed 规则影响首页公告 src 白名单 → 失效首页共享段缓存
    crate::http::invalidate_home_cache(&**state).await;
    Ok(ok(serde_json::json!({
        "dry_run": false,
        "kind": "embed",
        "pack_id": head.pack_id,
        "applied": { "video_embed_rules": applied },
    })))
}

async fn import_taxonomy(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let data = parse_taxonomy(&head.payload)?;
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(internal)?;
    let conflicts = referenced_categories(&mut tx, &data.cats).await?;
    if !conflicts.is_empty() {
        return Err(DomainError::Validation(format!(
            "存在被种子引用的分类改动，已拒绝：{}",
            conflicts.join("；")
        )));
    }
    if !confirm {
        let cur_cats: i64 =
            sqlx::query_scalar("SELECT count(*) FROM categories")
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(0);
        tx.rollback().await.ok();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": "taxonomy",
            "pack_id": head.pack_id,
            "will_change": data.cats.len() + data.kinds.len(),
            "current_categories": cur_cats,
            "pack_categories": data.cats.len(),
        })));
    }
    let snapshot = snapshot_taxonomy(&mut tx).await?;
    let n = apply_taxonomy(&mut tx, &data).await?;
    upsert_pack_row(&mut tx, actor, head, snapshot).await?;
    audit(
        &mut tx,
        *actor,
        "content_pack:import",
        &serde_json::json!({
            "pack_id": head.pack_id, "kind": head.kind,
            "name": head.name, "version": head.version,
        }),
    )
    .await;
    tx.commit().await.map_err(internal)?;
    state.module_flags.invalidate().await;
    invalidate_cache(state, "appearance").await;
    Ok(ok(serde_json::json!({
        "dry_run": false,
        "kind": "taxonomy",
        "pack_id": head.pack_id,
        "applied": { "categories": n },
    })))
}

/// rules 包导入（M3 §4.2）：payload.rules = {rule_键: 表达式}。
/// 键白名单（rule_spec_for）+ 表达式 lint 两道闸——预检与确认两态都全量校验；
/// 快照存导入前的键值（含空缺 = 回滚时删除，A2）。
async fn import_rules(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let Some(rules) =
        head.payload.get("rules").and_then(Value::as_object)
    else {
        return Err(bad("rules 包缺少 payload.rules"));
    };
    // 双闸校验：键白名单 + 表达式 lint（空值 = 清除该规则，允许）
    let mut errors: Vec<(String, String)> = Vec::new();
    for (key, expr_v) in rules {
        let Some(spec) = super::pack_format::rule_spec_for(key) else {
            errors.push((
                key.clone(),
                "rules 包不允许触碰该规则键".into(),
            ));
            continue;
        };
        let expr = expr_v.as_str().unwrap_or_default().trim();
        if !expr.is_empty() {
            if let Err(e) = crate::rules_engine::lint(expr, &spec) {
                errors.push((key.clone(), e.to_string()));
            }
        }
    }
    if !errors.is_empty() {
        return Err(DomainError::FieldErrors(errors));
    }
    let keys: Vec<String> = rules.keys().cloned().collect();
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(internal)?;
    if !confirm {
        let mut will = 0usize;
        for (key, expr_v) in rules {
            let cur: Option<String> = sqlx::query_scalar(
                "SELECT value FROM site_settings WHERE name = $1",
            )
            .bind(key)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal)?;
            if cur.as_deref() != Some(expr_v.as_str().unwrap_or("")) {
                will += 1;
            }
        }
        tx.rollback().await.ok();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": "rules",
            "pack_id": head.pack_id,
            "will_change": will,
        })));
    }
    // 快照：导入前键值（含空缺键 → 回滚删除）
    let cur: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name = ANY($1)",
    )
    .bind(&keys)
    .fetch_all(&mut *tx)
    .await
    .map_err(internal)?;
    let mut snap = serde_json::Map::new();
    for (k, v) in &cur {
        snap.insert(k.clone(), Value::String(v.clone()));
    }
    let snapshot = serde_json::json!({ "rules": Value::Object(snap) });
    let mut changed = Vec::new();
    for (key, expr_v) in rules {
        let expr = expr_v.as_str().unwrap_or_default();
        sqlx::query(
            "INSERT INTO site_settings (name, value, descr, grp) \
             VALUES ($1, $2, '规则公式（规则包）', 'module') \
             ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
             updated_at = now()",
        )
        .bind(key)
        .bind(expr)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
        changed.push(key.clone());
    }
    upsert_pack_row(&mut tx, actor, head, snapshot).await?;
    audit(
        &mut tx,
        *actor,
        "content_pack:import",
        &serde_json::json!({
            "pack_id": head.pack_id, "kind": head.kind,
            "name": head.name, "version": head.version,
        }),
    )
    .await;
    tx.commit().await.map_err(internal)?;
    invalidate_cache(state, "module").await;
    Ok(ok(serde_json::json!({
        "dry_run": false,
        "kind": "rules",
        "pack_id": head.pack_id,
        "applied": { "rules": changed.len() },
        "changed": changed,
    })))
}

/// assets 包导入（M1 增量）：payload.tables = {medals:[...], avatar_frames:[...]}。
/// 白名单表 + 快照重放（同 taxonomy 口径）。
async fn import_assets(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    // 键白名单预检（dry-run 与 confirm 共用）
    if let Some(tables) =
        head.payload.get("tables").and_then(Value::as_object)
    {
        for t in tables.keys() {
            if !super::pack_format::ASSET_TABLES.contains(&t.as_str()) {
                return Err(DomainError::Validation(format!(
                    "assets 包不允许触碰表：{t}"
                )));
            }
        }
    } else {
        return Err(bad("assets 包缺少 payload.tables"));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(internal)?;
    if !confirm {
        let medals = head
            .payload
            .pointer("/tables/medals")
            .and_then(Value::as_array)
            .map(|a| a.len())
            .unwrap_or(0);
        let frames = head
            .payload
            .pointer("/tables/avatar_frames")
            .and_then(Value::as_array)
            .map(|a| a.len())
            .unwrap_or(0);
        tx.rollback().await.ok();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": "assets",
            "pack_id": head.pack_id,
            "will_change": medals + frames,
            "medals": medals,
            "avatar_frames": frames,
        })));
    }
    let snapshot = snapshot_assets(&mut tx).await?;
    let (n_medals, n_frames) =
        apply_assets(&mut tx, &head.payload).await?;
    upsert_pack_row(&mut tx, actor, head, snapshot).await?;
    audit(
        &mut tx,
        *actor,
        "content_pack:import",
        &serde_json::json!({
            "pack_id": head.pack_id, "kind": head.kind,
            "name": head.name, "version": head.version,
        }),
    )
    .await;
    tx.commit().await.map_err(internal)?;
    invalidate_cache(state, "module").await;
    Ok(ok(serde_json::json!({
        "dry_run": false,
        "kind": "assets",
        "pack_id": head.pack_id,
        "applied": {
            "medals": n_medals, "avatar_frames": n_frames,
        },
    })))
}

async fn import_theme(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let Some(settings) =
        head.payload.get("settings").and_then(Value::as_object)
    else {
        return Err(bad("theme 包缺少 payload.settings"));
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(internal)?;
    let names: Vec<String> = settings.keys().cloned().collect();
    let metas: Vec<MetaRow> = sqlx::query_as(&format!(
        "{} WHERE s.name = ANY($1)",
        super::meta::META_SELECT
    ))
    .bind(&names)
    .fetch_all(&mut *tx)
    .await
    .map_err(internal)?;
    let by_name: std::collections::HashMap<String, MetaRow> = metas
        .into_iter()
        .map(|m| (m.name.clone(), m))
        .collect();
    if !confirm {
        // 预检：白名单 + 校验，不落库
        let mut errors: Vec<(String, String)> = Vec::new();
        let mut will = 0usize;
        for name in &names {
            if !THEME_KEYS.contains(&name.as_str()) {
                errors.push((
                    name.clone(),
                    "theme 包不允许触碰该设置键".into(),
                ));
                continue;
            }
            let Some(meta) = by_name.get(name) else {
                errors.push((name.clone(), "设置键不存在".into()));
                continue;
            };
            let raw = settings[name].as_str().unwrap_or_default();
            match validate_field(meta, raw) {
                Ok(v) if v != meta.value => will += 1,
                Ok(_) => {}
                Err(e) => errors.push((name.clone(), e)),
            }
        }
        tx.rollback().await.ok();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": "theme",
            "pack_id": head.pack_id,
            "will_change": will,
            "errors": errors,
        })));
    }
    let snapshot = snapshot_theme(&mut tx).await?;
    let (changed, errors) =
        apply_theme_import(&mut tx, settings, &by_name, false).await?;
    if !errors.is_empty() {
        return Err(DomainError::FieldErrors(errors));
    }
    upsert_pack_row(&mut tx, actor, head, snapshot).await?;
    audit(
        &mut tx,
        *actor,
        "content_pack:import",
        &serde_json::json!({
            "pack_id": head.pack_id, "kind": head.kind,
            "name": head.name, "version": head.version,
        }),
    )
    .await;
    tx.commit().await.map_err(internal)?;
    invalidate_cache(state, "appearance").await;
    Ok(ok(serde_json::json!({
        "dry_run": false,
        "kind": "theme",
        "pack_id": head.pack_id,
        "applied": { "settings": changed.len() },
        "changed": changed,
    })))
}

/// 同 pack_id 再导入 = 升级：快照滚动覆盖（回滚永远回到「本次导入前」）
/// payload 一并存表（0183）：rules 回滚只删包内键的边界依据
async fn upsert_pack_row(
    tx: &mut sqlx::PgTransaction<'_>,
    actor: &i64,
    head: &PackHead,
    snapshot: Value,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO content_packs \
         (pack_id, kind, name, version, core_compat, snapshot, payload, applied_by) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (pack_id) DO UPDATE SET kind = EXCLUDED.kind, \
         name = EXCLUDED.name, version = EXCLUDED.version, \
         core_compat = EXCLUDED.core_compat, snapshot = EXCLUDED.snapshot, \
         payload = EXCLUDED.payload, \
         applied_at = now(), applied_by = EXCLUDED.applied_by",
    )
    .bind(&head.pack_id)
    .bind(&head.kind)
    .bind(&head.name)
    .bind(&head.version)
    .bind(&head.core_compat)
    .bind(snapshot) // sqlx 的 Value 直映 jsonb
    .bind(&head.payload)
    .bind(actor)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(())
}

async fn audit(
    tx: &mut sqlx::PgTransaction<'_>,
    actor: i64,
    action: &str,
    detail: &Value,
) {
    let r = sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, ref) \
         VALUES (nextval('audit_log_id_seq'), $1, $2, $3::jsonb)",
    )
    .bind(actor)
    .bind(action)
    .bind(detail.to_string())
    .execute(&mut **tx)
    .await;
    if r.is_err() {
        tracing::error!("内容包审计写入失败（§5.7）");
    }
}

// ============ 回滚 ============

pub(super) async fn rollback(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    id: i64,
) -> DomainResult<HttpResponse> {
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT pack_id, kind, snapshot::text FROM content_packs \
         WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    let Some((pack_id, kind, snapshot)) = row else {
        return Err(DomainError::Validation("内容包记录不存在".into()));
    };
    let snapshot: Value = serde_json::from_str(&snapshot)
        .map_err(|e| DomainError::Internal(e.into()))?;
    // R11：回滚需要知道「包写过哪些键」——从登记的 payload 读取
    // （0183 起随导入落表；旧登记行 payload 为 NULL 时按旧口径全删回内置）
    let pack: Value = sqlx::query_scalar::<_, Option<String>>(
        "SELECT payload::text FROM content_packs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .unwrap_or(None)
    .flatten()
    .and_then(|t| serde_json::from_str(&t).ok())
    .unwrap_or(Value::Null);
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(internal)?;
    if kind == "taxonomy" {
        // 快照 dict 带 sort；转回 payload 形状走同一落库路径
        let payload = snapshot_to_payload(&snapshot);
        let data = parse_taxonomy(&payload)?;
        apply_taxonomy_guarded(&mut tx, &data).await?;
    } else if kind == "theme" {
        let Some(settings) =
            snapshot.get("settings").and_then(Value::as_object)
        else {
            return Err(DomainError::Validation(
                "快照损坏（缺少 settings 节）".into(),
            ));
        };
        let (_, errors) =
            apply_theme_rollback(&mut tx, settings).await?;
        if !errors.is_empty() {
            return Err(DomainError::FieldErrors(errors));
        }
    } else if kind == "rules" {
        // 快照 rules 只含导入前**已存在**的键；空快照 = 全部删除（回到内置默认）
        let snap_rules = snapshot
            .get("rules")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let cur_keys: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM site_settings WHERE name LIKE 'rule\\_%'",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(internal)?;
        // R11 修复：只删「本包导入写入的键」——导入后管理员手工新增的
        // rule_% 键不属于包，回滚不得误删。旧登记行（payload NULL）按旧口径。
        let packed_keys: Vec<String> = if pack.is_null() {
            cur_keys.clone()
        } else {
            pack.get("rules")
                .and_then(Value::as_object)
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default()
        };
        for key in cur_keys {
            if !snap_rules.contains_key(&key)
                && packed_keys.contains(&key)
            {
                sqlx::query("DELETE FROM site_settings WHERE name = $1")
                    .bind(&key)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
        }
        for (key, val) in &snap_rules {
            sqlx::query(
                "INSERT INTO site_settings (name, value) \
                 VALUES ($1, $2) ON CONFLICT (name) DO UPDATE \
                 SET value = EXCLUDED.value, updated_at = now()",
            )
            .bind(key)
            .bind(val.as_str().unwrap_or_default())
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        }
    } else if kind == "assets" {
        // 快照即 payload 形状（tables 原样），直接重放
        apply_assets(&mut tx, &snapshot).await?;
    } else {
        return Err(DomainError::Validation("未知包类别".into()));
    }
    sqlx::query("DELETE FROM content_packs WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    audit(
        &mut tx,
        *actor,
        "content_pack:rollback",
        &serde_json::json!({ "pack_id": pack_id, "kind": kind }),
    )
    .await;
    tx.commit().await.map_err(internal)?;
    state.module_flags.invalidate().await;
    invalidate_cache(state, "appearance").await;
    Ok(ok(serde_json::json!({ "rolled_back": pack_id })))
}
