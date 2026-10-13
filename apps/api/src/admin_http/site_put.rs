//! PUT /admin/settings（旧版单项写）：从 admin_http/site.rs 按域拆出
//! （300 行门禁）。0354 起支持可选乐观并发（expected updated_at）。

use actix_web::{put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

#[derive(Deserialize)]
pub(crate) struct SettingPut {
    pub(crate) name: String,
    pub(crate) value: String,
    /// 乐观并发（可选，与 /admin/settings/groups 同款灰度口径）：
    /// 客户端读取该键时的 updated_at（RFC3339）；缺省 = 不校验
    #[serde(default)]
    pub(crate) expected: Option<String>,
}

/// 修改单项站点设定（仅 sysop）。值相同 → 幂等成功（不刷 updated_at、
/// 不记审计）；expected 携带旧版本戳 → 5003 VersionConflict（409）
#[put("/admin/settings")]
pub(crate) async fn site_settings_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SettingPut>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.name.trim().is_empty() || body.value.len() > 4096 {
        return Err(DomainError::Validation("非法的设定项".into()));
    }
    // 三审 B-4：site_type 是站型包 apply 的派生锚点——设置页直改只改字符串，
    // 分类/模块/维度全不跟随，设置值与站点形态脱钩。拒绝并引导走站型切换。
    if body.name == "site_type" {
        return Err(DomainError::Validation(
            "站点类型请在后台「分类管理 → 站点类型包」切换（会完整应用分类/模块/维度）"
                .into(),
        ));
    }
    // 乐观并发（0354）：expected 带了版本戳才比对——值相同跳过在前，
    // 没改动的键不触发冲突；校验复用 groups 端点同一函数（灰度兼容）
    let name = body.name.trim();
    let current: Option<(String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT value, updated_at FROM site_settings \
             WHERE name = $1 FOR UPDATE",
        )
        .bind(name)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((current_value, current_at)) = current else {
        return Err(DomainError::Validation("设定项不存在".into()));
    };
    if current_value != body.value.trim() {
        if let Some(got) = &body.expected {
            let mut map = std::collections::HashMap::new();
            map.insert(name.to_string(), got.clone());
            crate::settings_http::expected::check_expected(
                &map,
                name,
                current_at,
            )?;
        }
    }
    let updated = sqlx::query(
        "UPDATE site_settings SET value = $3, updated_at = now() \
         WHERE name = $2 AND value IS DISTINCT FROM $3 RETURNING name",
    )
    .bind(auth.id)
    .bind(name)
    .bind(body.value.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 值未变（含旧版幂等写入）：不刷 updated_at、不记审计，直接成功
    if updated.is_none() {
        return Ok(ok(serde_json::json!({ "ok": true })));
    }
    state
        .repo
        .audit(Some(auth.id), "site_setting_update", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
