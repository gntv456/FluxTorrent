//! 内容包子导入步骤（2026-11-28 从 pack_import.rs 拆出守行数门禁）：
//! embed / taxonomy / rules / assets / theme 五个子导入 + upsert_pack_row
//! 落库原语 + audit 审计。事务编排在 pack_import.rs（快照→落库→登记→
//! 提交/回退），本件只实现各 kind 的纯覆盖路径；rollback 不经过本件
//! （读快照走逆向导入，不经子导入步骤）。
use actix_web::{web, HttpResponse};
use serde_json::Value;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::engine::validate_field;
use super::groups::invalidate_cache;
use super::meta::MetaRow;
use super::pack_assets;
use super::pack_embed;
use super::pack_format::{bad, parse_taxonomy, PackHead, THEME_KEYS};
use super::pack_store;
use super::pack_store::{
    apply_taxonomy, apply_theme_import, referenced_categories,
    snapshot_taxonomy, snapshot_theme,
};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// embed 包导入（0190）：payload.rules = [video_embed_rules 行数组]。
/// 白名单只有 video_embed_rules 一张表；校验/落库/快照/回滚都在
/// pack_embed.rs，与后台「视频内嵌规则」CRUD 共用同一道 `validate_body`。
/// 预检与确认共用同一次 `check_rows`，两态不可能口径分叉；
/// builtin 行不受包影响（要覆写即整包拒绝，0176：内置种子跟随版本）。
pub(super) async fn import_embed(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let Some(rows) = head.payload.get("rules").and_then(Value::as_array) else {
        return Err(bad("embed 包缺少 payload.rules"));
    };
    let rows = rows.clone();
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let bodies = match pack_embed::check_rows(&mut tx, &rows).await {
        Ok(v) => v,
        Err(e) => {
            tx.rollback().await.ok();
            return Err(e);
        }
    };
    if !confirm {
        tx.rollback().await.ok();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": "embed",
            "pack_id": head.pack_id,
            "will_change": bodies.len(),
        })));
    }
    let snapshot = pack_embed::snapshot(&mut tx).await?;
    let applied = pack_embed::replay(&mut tx, &bodies).await?;
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

pub(super) async fn import_taxonomy(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let data = parse_taxonomy(&head.payload)?;
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
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
    crate::cfgver::bump(state.get_ref(), "modules").await;
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
pub(super) async fn import_rules(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let Some(rules) = head.payload.get("rules").and_then(Value::as_object)
    else {
        return Err(bad("rules 包缺少 payload.rules"));
    };
    // 双闸校验：键白名单 + 表达式 lint（空值 = 清除该规则，允许）
    let mut errors: Vec<(String, String)> = Vec::new();
    for (key, expr_v) in rules {
        let Some(spec) = super::pack_format::rule_spec_for(key) else {
            errors.push((key.clone(), "rules 包不允许触碰该规则键".into()));
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
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
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
    let before: std::collections::HashMap<&str, &str> =
        cur.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
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
        // 只报「真的变了」的键：预检的 will_change 按差异计数，确认侧原先
        // 无条件 push，同一张包两态数字对不上（界面据此提示「已改 N 项」）
        if before.get(key.as_str()).copied() != Some(expr) {
            changed.push(key.clone());
        }
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
/// 白名单表 + 快照重放（同 taxonomy 口径）；行值过后台道具 CRUD 同一道闸
/// （范围校验 + CSS/图片净化），落库与快照在 pack_assets.rs。
pub(super) async fn import_assets(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    head: &PackHead,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    // 键白名单预检（dry-run 与 confirm 共用）
    if let Some(tables) = head.payload.get("tables").and_then(Value::as_object)
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
    let payload = match pack_assets::check_input(&head.payload) {
        Ok(v) => v,
        Err(errors) => return Err(DomainError::FieldErrors(errors)),
    };
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let declared: Vec<String> = payload
        .get("tables")
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();
    if !confirm {
        let count = |t: &str| {
            payload
                .pointer(&format!("/tables/{t}"))
                .and_then(Value::as_array)
                .map(|a| a.len())
                .unwrap_or(0)
        };
        let (medals, frames) = (count("medals"), count("avatar_frames"));
        tx.rollback().await.ok();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": "assets",
            "pack_id": head.pack_id,
            "will_change": medals + frames,
            "medals": medals,
            "avatar_frames": frames,
            // 包没声明的表不会被碰（旧写法把缺键当空集 = 整表清空）
            "tables_untouched": super::pack_format::ASSET_TABLES
                .iter()
                .filter(|t| !declared.iter().any(|d| d == *t))
                .map(|t| t.to_string())
                .collect::<Vec<_>>(),
        })));
    }
    let snapshot = pack_assets::snapshot_assets(&mut tx, &payload).await?;
    let (n_medals, n_frames) =
        pack_assets::apply_assets(&mut tx, &payload).await?;
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

pub(super) async fn import_theme(
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
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let names: Vec<String> = settings.keys().cloned().collect();
    let metas: Vec<MetaRow> = sqlx::query_as(&format!(
        "{} WHERE s.name = ANY($1)",
        super::meta::META_SELECT
    ))
    .bind(&names)
    .fetch_all(&mut *tx)
    .await
    .map_err(internal)?;
    let by_name: std::collections::HashMap<String, MetaRow> =
        metas.into_iter().map(|m| (m.name.clone(), m)).collect();
    if !confirm {
        // 预检：白名单 + 校验，不落库
        let mut errors: Vec<(String, String)> = Vec::new();
        let mut will = 0usize;
        for name in &names {
            if !THEME_KEYS.contains(&name.as_str()) {
                errors
                    .push((name.clone(), "theme 包不允许触碰该设置键".into()));
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
    let snapshot = snapshot_theme(&mut tx, &names).await?;
    let (changed, errors) =
        apply_theme_import(&mut tx, settings, &by_name).await?;
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
pub(super) async fn upsert_pack_row(
    tx: &mut sqlx::PgTransaction<'_>,
    actor: &i64,
    head: &PackHead,
    snapshot: Value,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO content_packs \
         (pack_id, kind, name, version, core_compat, snapshot, payload,
          applied_by) \
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

pub(super) async fn audit(
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
