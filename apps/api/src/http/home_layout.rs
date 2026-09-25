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

/// 首页板块清单（四审 L6：单源化）——键、推荐占宽、是否进默认排版，**只有这一份**。
/// 此前 Rust 侧只有一份键白名单、TS 侧另有三份（键白名单 / 默认排版 / 推荐占宽），
/// 任何一处漂移都表现为「后台能存但首页不认」，且 `latest` 长期没有渲染分支。
/// 前端渲染器表（key → 组件）本质是代码，留在 TS，由
/// `scripts/home_sections_guard.mjs` 比对两边键集防漂移（本地闸门与 CI 都挂）。
pub struct HomeSection {
    pub key: &'static str,
    /// 1/2/3 = 1/3、2/3、整行（前端 span=0 时用它）
    pub span: i32,
    /// 是否属于「未配置时的默认排版」；顺序即默认顺序
    pub in_default: bool,
}

pub const HOME_SECTIONS: &[HomeSection] = &[
    HomeSection {
        key: "news",
        span: 2,
        in_default: true,
    },
    HomeSection {
        key: "attendance",
        span: 1,
        in_default: true,
    },
    HomeSection {
        key: "shoutbox",
        span: 2,
        in_default: true,
    },
    HomeSection {
        key: "funbox",
        span: 1,
        in_default: true,
    },
    HomeSection {
        key: "resource_stats",
        span: 3,
        in_default: true,
    },
    HomeSection {
        key: "site_data",
        span: 2,
        in_default: true,
    },
    HomeSection {
        key: "lucky_draw",
        span: 1,
        in_default: true,
    },
    HomeSection {
        key: "links",
        span: 3,
        in_default: true,
    },
    // 海报墙：0089 起键就在白名单里，但前端 switch 一直没有分支，
    // 于是站长的排序/占宽对它无效（只在页面末尾硬渲染）。本次补上渲染位。
    HomeSection {
        key: "latest",
        span: 3,
        in_default: true,
    },
];

fn keys_joined() -> String {
    HOME_SECTIONS
        .iter()
        .map(|s| s.key)
        .collect::<Vec<_>>()
        .join("/")
}

fn is_known_key(k: &str) -> bool {
    HOME_SECTIONS.iter().any(|s| s.key == k)
}

/// 随 `/home` 下发给前端的清单（前端据此解析排版、算默认布局与占宽，不再自带副本）
pub fn home_sections_json() -> serde_json::Value {
    serde_json::Value::Array(
        HOME_SECTIONS
            .iter()
            .map(|s| {
                serde_json::json!({
                    "key": s.key, "span": s.span, "in_default": s.in_default,
                })
            })
            .collect(),
    )
}

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
        if !is_known_key(&it.key) {
            return Err(DomainError::Validation(format!(
                "未知板块键 {}（可用：{}）",
                it.key,
                keys_joined()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_keys_unique() {
        let mut v: Vec<&str> = HOME_SECTIONS.iter().map(|s| s.key).collect();
        let n = v.len();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), n, "首页板块键重复");
    }

    /// 清单纪律：span 只能是 1/2/3；latest 必须在默认排版里（0089 起首页恒有海报墙）
    #[test]
    fn section_spans_and_latest_default() {
        for s in HOME_SECTIONS {
            assert!(
                [1, 2, 3].contains(&s.span),
                "{} 的推荐占宽非法: {}",
                s.key,
                s.span
            );
        }
        let latest = HOME_SECTIONS
            .iter()
            .find(|s| s.key == "latest")
            .expect("latest 键必须在清单里");
        assert!(latest.in_default, "默认首页应含海报墙");
    }

    /// 下发形状契约：前端 home-layout.tsx 按 key/span/in_default 三字段消费
    #[test]
    fn sections_json_shape() {
        let v = home_sections_json();
        let arr = v.as_array().expect("数组");
        assert_eq!(arr.len(), HOME_SECTIONS.len());
        for (i, it) in arr.iter().enumerate() {
            assert_eq!(it["key"].as_str(), Some(HOME_SECTIONS[i].key));
            assert!(it["span"].is_number());
            assert!(it["in_default"].is_boolean());
        }
    }
}
