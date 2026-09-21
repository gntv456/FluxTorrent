//! 自定义菜单（第五轮 P2）：导航项管理与公开菜单。
//! 从 admin_p2_http.rs 按域拆出。

use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::menus_public::MenuItemRow;
use super::staff;

pub async fn menu_settings(db: &sqlx::PgPool) -> (bool, i32) {
    let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT name, \
         value FROM site_settings WHERE name IN ('nav.custom_enabled','nav.min_visible_class')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let mut enabled = false;
    let mut min_class = 0;
    for (k, v) in rows {
        match k.as_str() {
            "nav.custom_enabled" => {
                enabled =
                    v == "1" || v.eq_ignore_ascii_case("true") || v == "yes"
            }
            "nav.min_visible_class" => min_class = v.parse().unwrap_or(0),
            _ => {}
        }
    }
    (enabled, min_class)
}

#[get("/admin/menu-items")]
pub async fn menu_items_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows: Vec<MenuItemRow> = sqlx::query_as(
        "SELECT id, location, label, url, parent_id, target, min_class, sort, enabled \
         FROM menu_items ORDER BY location, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (custom_enabled, min_visible_class) =
        menu_settings(&state.repo.db).await;
    Ok(ok(serde_json::json!({
        "items": rows,
        "custom_enabled": custom_enabled,
        "min_visible_class": min_visible_class,
    })))
}

/// 菜单全局开关设置
#[derive(Deserialize)]
struct MenuSettingsReq {
    #[serde(default)]
    custom_enabled: Option<bool>,
    #[serde(default)]
    min_visible_class: Option<i32>,
}

#[put("/admin/menu-settings")]
pub async fn menu_settings_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MenuSettingsReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // 写 site_settings 等价于改站点设定：须过 SETTINGS_MANAGE（99 档），
    // 与 settings_groups_put 同口径；旧版仅 staff() 把 99 档收窄放宽到 90。
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if let Some(en) = body.custom_enabled {
        sqlx::query(
            "UPDATE site_settings SET value = $1, \
         updated_at = now() WHERE name = 'nav.custom_enabled'",
        )
        .bind(if en { "1" } else { "0" })
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    if let Some(mc) = body.min_visible_class {
        if !(0..=99).contains(&mc) {
            return Err(DomainError::Validation(
                "min_visible_class 取值 0-99".into(),
            ));
        }
        sqlx::query(
            "UPDATE site_settings SET value = $1, \
         updated_at = now() WHERE name = 'nav.min_visible_class'",
        )
        .bind(mc.to_string())
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "menu_settings.update", None)
        .await;
    let (en, mc) = menu_settings(&state.repo.db).await;
    Ok(ok(
        serde_json::json!({ "custom_enabled": en, "min_visible_class": mc }),
    ))
}

#[derive(Deserialize)]
pub(super) struct MenuItemReq {
    #[serde(default)]
    pub(super) location: Option<String>,
    #[serde(default)]
    pub(super) label: Option<String>,
    #[serde(default)]
    pub(super) url: Option<String>,
    #[serde(default)]
    pub(super) parent_id: Option<i64>,
    #[serde(default)]
    pub(super) target: Option<String>,
    #[serde(default)]
    pub(super) min_class: Option<i32>,
    #[serde(default)]
    pub(super) sort: Option<i32>,
    #[serde(default)]
    pub(super) enabled: Option<bool>,
}

/// 校验 target / parent_id（parent 须存在且同 location）
pub async fn menu_item_validate(
    db: &sqlx::PgPool,
    body: &MenuItemReq,
    self_id: Option<i64>,
) -> DomainResult<()> {
    if let Some(t) = &body.target {
        if !["_self", "_blank"].contains(&t.as_str()) {
            return Err(DomainError::Validation(
                "target 取值 _self/_blank".into(),
            ));
        }
    }
    if let Some(mc) = body.min_class {
        if !(0..=99).contains(&mc) {
            return Err(DomainError::Validation("min_class 取值 0-99".into()));
        }
    }
    if let Some(pid) = body.parent_id {
        if pid > 0 {
            if Some(pid) == self_id {
                return Err(DomainError::Validation("父菜单不能是自己".into()));
            }
            if let Some(sid) = self_id {
                if pid == sid {
                    return Err(DomainError::Validation(
                        "父菜单不能是自己".into(),
                    ));
                }
                // 禁止把自己的后代设为父（成环）
                let child_cnt: i64 = sqlx::query_scalar(
                                        "WITH RECURSIVE sub AS ( SELECT id \
                     FROM menu_items WHERE parent_id = $1 UNION ALL SELECT m.id \
                     FROM menu_items m JOIN sub s ON m.parent_id = s.id ) \
                     SELECT count(*) FROM sub WHERE id = $2",
                )
                .bind(sid)
                .bind(pid)
                .fetch_one(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                if child_cnt > 0 {
                    return Err(DomainError::Validation(
                        "不能把自己的子菜单设为父菜单".into(),
                    ));
                }
            }
            let row: Option<(i64, String)> = sqlx::query_as(
                "SELECT id, location FROM menu_items WHERE id = $1",
            )
            .bind(pid)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            match row {
                None => {
                    return Err(DomainError::Validation("父菜单不存在".into()))
                }
                Some((_, ploc)) => {
                    let loc = body.location.clone().unwrap_or_default();
                    if !loc.is_empty() && ploc != loc {
                        return Err(DomainError::Validation(
                            "父菜单须在同一定位".into(),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}
