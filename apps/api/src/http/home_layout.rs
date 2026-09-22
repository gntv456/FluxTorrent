//! 首页布局：raw 读取 + admin 写入。
//! 从 http.rs 按域拆出。

use super::auth_infra::require_auth;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use actix_web::{put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

/// 读取首页排版配置（0089）：返回原文；库错误/缺行回空串（首页永远可渲染）
pub async fn home_layout_raw(db: &sqlx::PgPool) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'home_layout'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or_default()
}

/// 首页板块键白名单（0089）：与前端 HomeSections 渲染分支一一对应
pub const HOME_SECTION_KEYS: &[&str] = &[
    "news",
    "attendance",
    "shoutbox",
    "funbox",
    "resource_stats",
    "site_data",
    "lucky_draw",
    "links",
    "latest",
];

#[derive(Deserialize)]
struct HomeLayoutItem {
    key: String,
    /// 1/2/3 = 1/3、2/3、整行；缺省 0 由前端按板块推荐档处理
    #[serde(default)]
    span: i32,
}

/// 保存首页排版（sysop，0089）：校验 JSON 结构 + 键白名单 + 去重 + span 白名单，
/// 规范化后存 site_settings.home_layout。items 传空数组 = 恢复默认排版。
#[put("/admin/home-layout")]
pub async fn admin_home_layout_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<Vec<HomeLayoutItem>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.len() > 20 {
        return Err(DomainError::Validation("板块数量至多 20".into()));
    }
    let mut norm: Vec<serde_json::Value> = Vec::new();
    let mut seen: std::collections::HashSet<&str> =
        std::collections::HashSet::new();
    for it in body.iter() {
        if !HOME_SECTION_KEYS.contains(&it.key.as_str()) {
            return Err(DomainError::Validation(format!(
                "未知板块键 {}（可用：{}）",
                it.key,
                HOME_SECTION_KEYS.join("/")
            )));
        }
        if !seen.insert(&it.key) {
            return Err(DomainError::Validation(format!(
                "板块 {} 重复",
                it.key
            )));
        }
        if ![0, 1, 2, 3].contains(&it.span) {
            return Err(DomainError::Validation(
                "span 取值 1/2/3（缺省自动）".into(),
            ));
        }
        norm.push(serde_json::json!({ "key": it.key, "span": it.span }));
    }
    let value = serde_json::to_string(&norm)
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
                "INSERT INTO site_settings (name, value, descr, grp) VALUES \
         ('home_layout', $1, '首页板块排版（JSON 数组，空 = 默认布局）', 'main') ON CONFLICT \
         (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(&value)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "home_layout.update", None)
        .await;
    // 首页共享缓存写失效（0152）：排版进 /home 共享段
    super::home::invalidate_home_cache(&state).await;
    Ok(ok(
        serde_json::json!({ "saved": norm.len(), "layout": norm }),
    ))
}
