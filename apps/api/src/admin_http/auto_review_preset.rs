//! 自动审种·预设套用（0400 批 2，从 auto_review_ops.rs 再拆，300 行门禁）：
//! 一键套用推荐/站型包/自定义三档预设，以及两者共用的设定写入助手
//! （`apply_overrides` 覆盖写组内键 + `upsert_setting` 单键写）。
//! 回放与按规则恢复留在 auto_review_ops.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

#[derive(Deserialize)]
struct ApplyReq {
    /// balanced / cautious / efficient / off（auto_review_presets 键）
    preset: Option<String>,
    /// movie / music / ...（auto_review_site_packs 键）
    site_pack: Option<String>,
}

#[post("/admin/auto-review/apply")]
async fn apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ApplyReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let db = &state.repo.db;
    let mut applied: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    if let Some(preset) = &body.preset {
        let overrides: Option<String> = sqlx::query_scalar(
            "SELECT overrides::text FROM auto_review_presets WHERE preset = $1",
        )
        .bind(preset)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(ov) = overrides else {
            return Err(DomainError::Validation(format!("未知预设：{preset}")));
        };
        apply_overrides(db, &ov, &mut skipped).await?;
        upsert_setting(db, "auto_review_preset", preset).await?;
        applied.push(format!("preset:{preset}"));
    }
    if let Some(pack) = &body.site_pack {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT title_template, overrides::text \
             FROM auto_review_site_packs WHERE site_type = $1",
        )
        .bind(pack)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some((template, ov)) = row else {
            return Err(DomainError::Validation(format!("未知站型包：{pack}")));
        };
        if !template.is_empty() {
            if setting_hand_edited(db, "auto_review_title_template").await? {
                skipped.push("auto_review_title_template".into());
            } else {
                upsert_setting(db, "auto_review_title_template", &template)
                    .await?;
            }
        }
        apply_overrides(db, &ov, &mut skipped).await?;
        upsert_setting(db, "auto_review_site_preset", pack).await?;
        applied.push(format!("site_pack:{pack}"));
    }
    if applied.is_empty() {
        return Err(DomainError::Validation(
            "body 须带 preset 或 site_pack".into(),
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "settings.auto_review.apply", None)
        .await;
    crate::settings_http::groups::invalidate_cache(&state, "auto_review").await;
    // 套包改写了组内键：ruleset 代际必须 +1（0400 §7），否则台账的
    // ruleset_ver 快照无法区分「套包前/后」的裁决代际
    crate::settings_http::ruleset::bump(db).await;
    Ok(ok(serde_json::json!({
        "applied": applied,
        "skipped": skipped,
    })))
}

fn default_days() -> i32 {
    30
}

/// POST /admin/auto-review/bulk-restore：按规则反查近期自动拒绝的种子，
/// 批量恢复回待审（§6 回滚：误杀应急 runbook 的机审侧入口）。
/// 只动 `approval_status=2 且 deny_reason 属自动字典（sort 91-96）` 的种，
/// 人工拒绝的永不触碰；每次操作留审计与留痕。
/// 0401 §18.5 升级安全承诺的落地：site_settings.updated_at 晚于包行
/// changed_at 的键视为「站长手改」，套包跳过不覆盖（回应用户改的键以
/// 站长为准）。updated_at 在保存链/本文件 upsert 时都会刷新，包行
/// changed_at 出厂即定，二者对比即可判定。
async fn setting_hand_edited(db: &PgPool, name: &str) -> DomainResult<bool> {
    let hand_edited: Option<bool> = sqlx::query_scalar(
        "SELECT s.updated_at > ANY(\
           SELECT p.changed_at FROM auto_review_site_packs p)\
         FROM site_settings s WHERE s.name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(hand_edited.unwrap_or(false))
}

async fn apply_overrides(
    db: &PgPool,
    overrides_json: &str,
    skipped: &mut Vec<String>,
) -> DomainResult<()> {
    let map: serde_json::Value =
        serde_json::from_str(overrides_json).map_err(|e| {
            DomainError::Validation(format!("覆盖项 JSON 无效：{e}"))
        })?;
    let Some(obj) = map.as_object() else {
        return Err(DomainError::Validation("覆盖项必须是对象".into()));
    };
    for (k, v) in obj {
        // 站长手改过的键不覆盖（0401 §18.5；跳过名单回给管理面）
        if setting_hand_edited(db, k).await? {
            skipped.push(k.clone());
            continue;
        }
        let val = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        upsert_setting(db, k, &val).await?;
    }
    Ok(())
}

async fn upsert_setting(
    db: &PgPool,
    name: &str,
    value: &str,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO site_settings (name, value, descr, grp) \
         VALUES ($1, $2, '自动审种（0400/0401 apply 写入）', 'auto_review') \
         ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value",
    )
    .bind(name)
    .bind(value)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}
