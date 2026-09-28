//! 视图布局（E6）：种子列表列显隐 + 详情页段落显隐的站点级配置。
//! 范式照抄 home_layout：单源清单（本文件）+ site_settings 单键 JSON +
//! sysop 保存端点 + site-profile 下发。**只做显隐不做顺序**——列表列序是
//! 油猴脚本/爬虫的 DOM 契约（pro_* 语义类同源），段落顺序维持站型默认。
//!
//! 与模块开关（MODULE_KEYS）的层级关系：模块关 = 整域硬下线（页面/导航/
//! 业务全消失），永远优先于本配置；本配置只裁「已启用域内部的展示位」。

use super::auth_infra::require_auth;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use actix_web::{get, put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

/// 列表列清单（key 默认显示）。selectable 复选列与 actions 行为列是调用方
/// 契约（批量下载/行操作），不在配置面内、永远渲染。
pub const TORRENT_COLUMNS: &[&str] = &[
    "cat",       // 类型色块/图标
    "cover",     // 封面
    "title",     // 标题三行（含促销/标签/发布者）
    "comments",  // 评论数
    "alive",     // 存活
    "size",      // 体积
    "seeders",   // 做种
    "leechers",  // 下载
    "completed", // 完成
];

/// 详情页段落清单。面包屑/海报头/规格网格/底部操作条为页面骨架不在配置面内；
/// descr/mediainfo/nfo/peers 等数据态段落「无数据自动不渲染」，配置是额外
/// 关闭位（关=有数据也不出）。
pub const DETAIL_SECTIONS: &[&str] = &[
    "descr",
    "mediainfo",
    "nfo",
    "peers",
    "group",
    "collections",
    "files",
    "subtitles",
    "snatches",
    "thankers",
    "infohash",
    "comments",
];

/// 键集下发（登录即可读）：编辑器据此渲染全部可配置项。
/// 键显示名走前端 i18n（viewLayout.names），本端点只出键。
#[get("/view-layout")]
pub async fn view_layout_keys(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    Ok(ok(serde_json::json!({
        "columns": TORRENT_COLUMNS,
        "sections": DETAIL_SECTIONS,
    })))
}

fn known(k: &str, list: &[&str]) -> bool {
    list.contains(&k)
}

fn keys_joined(list: &[&str]) -> String {
    list.join("/")
}

/// 读隐藏配置（E6）：返回两个集合的 JSON 串（{"columns":[…],"sections":[…]}）。
/// 缺行/解析失败回空对象——前端拿不到配置 = 全部显示（永不高墙）。
pub async fn view_hidden_raw(db: &sqlx::PgPool) -> serde_json::Value {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'view_hidden'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| serde_json::from_str(&v).ok())
    .unwrap_or_else(|| serde_json::json!({}))
}

#[derive(Deserialize)]
struct ViewHiddenBody {
    #[serde(default)]
    columns: Vec<String>,
    #[serde(default)]
    sections: Vec<String>,
}

/// 保存视图布局（sysop）：只存「隐藏项」集合，白名单校验后规范化。
/// 传空 = 全恢复默认（全部显示）。
#[put("/admin/view-hidden")]
pub async fn admin_view_hidden_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ViewHiddenBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.columns.len() > TORRENT_COLUMNS.len()
        || body.sections.len() > DETAIL_SECTIONS.len()
    {
        return Err(DomainError::Validation("隐藏项数量超限".into()));
    }
    for (items, list, what) in [
        (&body.columns, TORRENT_COLUMNS, "列"),
        (&body.sections, DETAIL_SECTIONS, "段落"),
    ] {
        let mut seen = std::collections::HashSet::new();
        for k in items {
            if !known(k, list) {
                return Err(DomainError::Validation(format!(
                    "未知{}键 {}（可用：{}）",
                    what,
                    k,
                    keys_joined(list)
                )));
            }
            if !seen.insert(k.as_str()) {
                return Err(DomainError::Validation(format!(
                    "{} {} 重复",
                    what, k
                )));
            }
        }
        // title 段/列隐藏会让种子无法点击——保留最低可用性
        if what == "列" && seen.contains("title") {
            return Err(DomainError::Validation(
                "标题列不可隐藏（列表最低可用性）".into(),
            ));
        }
    }
    let value = serde_json::json!({
        "columns": body.columns, "sections": body.sections,
    });
    sqlx::query(
        "INSERT INTO site_settings (name, value, descr, grp) VALUES \
         ('view_hidden', $1, '视图布局隐藏项（JSON，空 = 全默认显示）', 'main') \
         ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
         updated_at = now()",
    )
    .bind(value.to_string())
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "view_hidden.update", None)
        .await;
    Ok(ok(serde_json::json!({ "saved": true, "view": value })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_keys_unique() {
        let mut v = TORRENT_COLUMNS.to_vec();
        let n = v.len();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), n, "列表列键重复");
    }

    #[test]
    fn section_keys_unique() {
        let mut v = DETAIL_SECTIONS.to_vec();
        let n = v.len();
        v.sort_unstable();
        v.dedup();
        assert_eq!(v.len(), n, "详情段落键重复");
    }

    /// title 必须在列清单里（不可隐藏的最低可用列）
    #[test]
    fn title_column_present() {
        assert!(TORRENT_COLUMNS.contains(&"title"));
    }
}
