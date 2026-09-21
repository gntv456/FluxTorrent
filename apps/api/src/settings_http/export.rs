//! 导出 / 导入（P2 §4.3）：全量配置 JSON + diff 空跑 + confirm 落库。
//! 从 settings_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use std::collections::HashMap;

use super::engine::validate_field;
use super::groups::{effects_for, invalidate_cache, push_unique, truncate};
use super::meta::MAX_BATCH;
use super::meta::{MetaRow, META_SELECT};

// 导出 / 导入（P2 §4.3）
// ============================================================================
//
// 导出：全量配置 JSON。密文字段默认掩码（值为空串并列入 masked_secrets），
//       `plaintext=1` 时以明文导出（仅 sysop）。
// 导入：仅 sysop。首次调用（confirm 缺省）为空跑，只回字段级 diff；带
//       `confirm=true` 才落库。逐字段复用同一套 validate_field。
//       密文空值语义随 `plaintext` 开关而定：
//         - plaintext=false（默认）：空值 = 保持不变，与「导出默认掩码」一致，
//           避免把掩码快照里的空串误当成"清空密钥"。
//         - plaintext=true：空值 = 按字面还原为空，保证
//           「plaintext 导出 → 清库 → 导入 → 再导出」严格 diff 一致。
//       导出快照自带 `plaintext` 字段，前端解析后原样回传即可。

#[derive(Deserialize)]
struct ExportQ {
    /// 1 = 密文字段以明文导出（仅 sysop）
    #[serde(default)]
    plaintext: u8,
}

#[get("/admin/settings/export")]
pub(super) async fn settings_export(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ExportQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let plaintext = q.plaintext == 1;
    if plaintext {
        crate::authz::require_perm(
            &state,
            &auth,
            crate::authz::perm::SETTINGS_MANAGE,
        )
        .await?;
    }
    let rows: Vec<MetaRow> = sqlx::query_as(&format!(
        "{META_SELECT} ORDER BY s.grp, m.group_key, m.card_order, s.name"
    ))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut settings = serde_json::Map::new();
    let mut masked: Vec<String> = Vec::new();
    for m in &rows {
        if m.secret && !plaintext {
            settings.insert(
                m.name.clone(),
                serde_json::Value::String(String::new()),
            );
            masked.push(m.name.clone());
        } else {
            settings.insert(
                m.name.clone(),
                serde_json::Value::String(m.value.clone()),
            );
        }
    }
    let count = settings.len();
    Ok(ok(serde_json::json!({
        "format": "fluxtorrent.settings",
        "version": 1,
        "exported_at": chrono::Utc::now(),
        "plaintext": plaintext,
        "count": count,
        "masked_secrets": masked,
        "settings": settings,
    })))
}

#[derive(Deserialize)]
struct ImportReq {
    settings: HashMap<String, String>,
    #[serde(default)]
    confirm: bool,
    /// 快照是否来自明文导出（`plaintext: true`）：决定密文空值的语义，
    /// 见模块顶部注释。缺省 false = 空值保持不变（安全默认）。
    #[serde(default)]
    plaintext: bool,
}

#[post("/admin/settings/import")]
pub(super) async fn settings_import(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ImportReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 导入仅 sysop（§4.3）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.settings.is_empty() {
        return Err(DomainError::Validation("导入内容为空".into()));
    }
    if body.settings.len() > MAX_BATCH * 4 {
        return Err(DomainError::Validation("导入条目过多".into()));
    }

    let names: Vec<String> = body.settings.keys().cloned().collect();
    let metas: Vec<MetaRow> =
        sqlx::query_as(&format!("{META_SELECT} WHERE s.name = ANY($1)"))
            .bind(&names)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let by_name: HashMap<String, MetaRow> =
        metas.into_iter().map(|m| (m.name.clone(), m)).collect();

    let mut unknown: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<(String, String, String)> = Vec::new(); // (name, old, new)

    for name in &names {
        let Some(meta) = by_name.get(name) else {
            unknown.push(name.clone());
            continue;
        };
        let raw = &body.settings[name];
        if meta.readonly {
            // 只读项（版本信息等）会随导出快照一并带回：导入时静默跳过而非报错
            skipped.push(name.clone());
            continue;
        }
        if meta.secret && raw.trim().is_empty() && !body.plaintext {
            continue; // 掩码快照中的密文空值 = 保持不变（明文快照则按字面还原）
        }
        match validate_field(meta, raw) {
            Ok(normalized) => {
                if normalized != meta.value {
                    pending.push((
                        name.clone(),
                        meta.value.clone(),
                        normalized,
                    ));
                }
            }
            Err(e) => errors.push((name.clone(), e)),
        }
    }
    if !errors.is_empty() {
        return Err(DomainError::FieldErrors(errors));
    }

    // 空跑：只回 diff，不落库（「导入后强制 diff 确认」）
    if !body.confirm {
        let diff: Vec<serde_json::Value> = pending
            .iter()
            .map(|(n, o, v)| {
                let secret = by_name[n].secret;
                serde_json::json!({
                    "name": n,
                    "group": by_name[n].grp,
                    "old": if secret { "••••".to_string() } else { truncate(o) },
                    "new": if secret { "••••".to_string() } else { truncate(v) },
                })
            })
            .collect();
        let changed_names: Vec<String> =
            pending.iter().map(|(n, _, _)| n.clone()).collect();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "unknown": unknown,
            "skipped_readonly": skipped,
            "will_change": pending.len(),
            "diff": diff,
            "effects": effects_for(&changed_names),
        })));
    }

    // ---- 确认落库：事务 + 逐项审计 ----
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut changed: Vec<String> = Vec::new();
    let mut groups: Vec<String> = Vec::new();
    for (name, _old, new) in &pending {
        let current: Option<String> = sqlx::query_scalar(
            "SELECT value FROM site_settings WHERE name = $1 FOR UPDATE",
        )
        .bind(name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let current = current.ok_or_else(|| {
            DomainError::Validation(format!("设定项不存在 {name}"))
        })?;
        if current == *new {
            continue;
        }
        sqlx::query(
            "UPDATE site_settings SET value = $2, \
         updated_at = now() WHERE name = $1",
        )
        .bind(name)
        .bind(new)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let meta = &by_name[name];
        let (audit_old, audit_new) = if meta.secret {
            ("已设置".to_string(), "已更新".to_string())
        } else {
            (truncate(&current), truncate(new))
        };
        sqlx::query(
            "INSERT INTO audit_log (id, actor_id, action, ref) \
             VALUES (nextval('audit_log_id_seq'), $1, 'setting:update', $2::jsonb)",
        )
        .bind(auth.id)
        .bind(
            serde_json::json!({
                "setting": name,
                "group": meta.grp,
                "old": audit_old,
                "new": audit_new,
                "via": "import",
            })
            .to_string(),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        push_unique(&mut groups, &meta.grp);
        changed.push(name.clone());
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    for g in &groups {
        invalidate_cache(&state, g).await;
    }
    Ok(ok(serde_json::json!({
        "dry_run": false,
        "unknown": unknown,
        "skipped_readonly": skipped,
        "applied": changed.len(),
        "changed": changed,
        "groups": groups,
        "effects": effects_for(&changed),
    })))
}
