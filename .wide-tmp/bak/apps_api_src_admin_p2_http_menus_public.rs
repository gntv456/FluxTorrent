//! 公开菜单（第五轮 P2）：GET /menu-items。
//! 从 admin_p2_http/menus.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::menus::menu_settings;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::optional_auth;
use crate::state::AppState;

#[get("/menu-items")]
pub async fn menu_items_public(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<MenuPublicQ>,
) -> DomainResult<HttpResponse> {
    if !["sidebar", "footer", "topbar"].contains(&q.location.as_str()) {
        return Err(DomainError::Validation(
            "location 取值 sidebar/footer/topbar".into(),
        ));
    }
    // 全局开关关闭 → 返回空（前端显式回退默认导航，不做静默混淆）
    let (custom_enabled, min_visible_class) =
        menu_settings(&state.repo.db).await;
    if !custom_enabled {
        return Ok(ok(Vec::<MenuItemRow>::new()));
    }
    // 可选鉴权：登录按等级过滤，匿名只看 min_class=0；整体门槛不过 → 空
    let user_class = optional_auth(&req, &state)
        .await
        .map(|u| u.class_id)
        .unwrap_or(0);
    if user_class < min_visible_class {
        return Ok(ok(Vec::<MenuItemRow>::new()));
    }
    // U1 §6.2：menu_items.module_key 非空的条目随模块开关过滤——
    // 无归属（NULL）或模块开启（module_xxx='yes'，未配置键视为关按 T3 需站长显式配置）
    // 的条目放行。子查询逐行判定，60 行上限下代价可忽略。
    let rows: Vec<MenuItemRow> = sqlx::query_as(
        "SELECT id, location, label, url, parent_id, target, min_class, sort, enabled \
         FROM menu_items m \
         WHERE enabled AND location = $1 AND min_class <= $2 \
           AND (m.module_key IS NULL OR COALESCE(( \
                 SELECT value = 'yes' FROM site_settings s \
                 WHERE s.name = 'module_' || m.module_key), false)) \
         ORDER BY sort, id LIMIT 60",
    )
    .bind(&q.location)
    .bind(user_class)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- 消息模板（message-templates）----

/// 前台：生效中的自定义菜单（按位置）
#[derive(Deserialize)]
struct MenuPublicQ {
    #[serde(default = "default_location")]
    location: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct MenuItemRow {
    id: i64,
    location: String,
    label: String,
    url: String,
    parent_id: i64,
    target: String,
    min_class: i32,
    sort: i32,
    enabled: bool,
}

pub(super) fn default_location() -> String {
    "sidebar".into()
}
