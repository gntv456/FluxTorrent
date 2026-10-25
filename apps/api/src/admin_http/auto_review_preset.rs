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
        apply_overrides(db, &ov).await?;
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
            upsert_setting(db, "auto_review_title_template", &template).await?;
        }
        apply_overrides(db, &ov).await?;
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
    Ok(ok(serde_json::json!({ "applied": applied })))
}

fn default_days() -> i32 {
    30
}

/// POST /admin/auto-review/bulk-restore：按规则反查近期自动拒绝的种子，
/// 批量恢复回待审（§6 回滚：误杀应急 runbook 的机审侧入口）。
/// 只动 `approval_status=2 且 deny_reason 属自动字典（sort 91-96）` 的种，
/// 人工拒绝的永不触碰；每次操作留审计与留痕。
async fn apply_overrides(
    db: &PgPool,
    overrides_json: &str,
) -> DomainResult<()> {
    let map: serde_json::Value =
        serde_json::from_str(overrides_json).map_err(|e| {
            DomainError::Validation(format!("覆盖项 JSON 无效：{e}"))
        })?;
    let Some(obj) = map.as_object() else {
        return Err(DomainError::Validation("覆盖项必须是对象".into()));
    };
    for (k, v) in obj {
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
