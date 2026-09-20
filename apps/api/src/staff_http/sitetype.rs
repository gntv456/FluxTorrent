//! 站点资料与站型包列表。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 公开：当前站点档案（类型包 + 分类 + 模块开关 + 品牌名），前端布局/导航/上传表单由此驱动
#[get("/site-profile")]
pub async fn site_profile(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let site_type: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_type'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "general".into());
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort, tagline FROM site_type_packs WHERE code = $1",
    ).bind(&site_type)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 实际分类以 categories 表为准（类型包只是初始快照，管理组可再编辑）
    let cats: Vec<(i32, String)> =
        sqlx::query_as("SELECT id, name FROM categories ORDER BY id")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let brand: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .flatten()
    .or(pack.as_ref().map(|p| p.brand.clone()))
    .unwrap_or_default();
    // 站点货币名（0082）：默认「魔力」，站长可后台改任意名；空值兜底回默认
    let currency: String = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'currency_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v: &String| !v.trim().is_empty())
    .unwrap_or_else(|| "魔力".to_string());
    // 建站日期（页脚版权条 "(c) 站名 日期 Powered by FluxTorrent" 用）
    let founded: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'datefounded'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v: &String| !v.trim().is_empty());

    // 模块开关：site_type_packs.modules 只是站型的**初始快照**，运行时权威在
    // site_settings.module_*（后台改了开关，导航要跟着变）。以前者打底、后者覆盖。
    let mut modules = pack
        .as_ref()
        .map(|p| p.modules.clone())
        .unwrap_or_else(|| serde_json::json!({}));
    let overrides: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name ~ '^module_'",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(obj) = modules.as_object_mut() {
        for (name, value) in overrides {
            if let Some(key) = name.strip_prefix("module_") {
                obj.insert(key.to_string(), serde_json::json!(value == "yes"));
            }
        }
    }
    // 元数据源（0087）：csv → 数组，控制上传页条目输入显隐与 PT-Gen 范围
    let sources_raw: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'metadata_sources'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    let sources: Vec<String> = sources_raw
        .unwrap_or_else(|| "imdb,douban,bangumi,indienova".into())
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect();
    // 登录页品牌区（0143/0145）：site_tagline = 站长覆盖值，空 = 动态跟随当前
    // 站型包默认（读 site_type JOIN packs.tagline）——设置卡直切站型即刻生效，
    // 不依赖 apply 向导物化；logo = 站长可配 URL
    let tagline: String = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_tagline'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
    .or_else(|| {
        pack.as_ref()
            .map(|p| p.tagline.trim().to_string())
            .filter(|t| !t.is_empty())
    })
    .unwrap_or_default();
    let site_logo: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_logo'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty());
    // 站点简介（0088）：页脚「站点信息」卡片文案，留空由前端回落字典默认
    let site_desc: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_desc'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty());
    Ok(ok(serde_json::json!({
        "site_type": site_type,
        "pack_name": pack.as_ref().map(|p| p.name.clone()),
        "brand": brand,
        "tagline": tagline,
        "site_logo": site_logo,
        "currency_name": currency,
        "founded": founded,
        "metadata_sources": sources,
        "site_desc": site_desc,
        "categories": cats.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "modules": modules,
    })))
}

/// 类型包列表（管理组：切换向导）
#[get("/admin/site-type-packs")]
pub async fn site_type_pack_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let rows: Vec<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort, tagline FROM site_type_packs ORDER BY sort",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct SiteTypePack {
    pub(super) code: String,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) brand: String,
    pub(super) categories: serde_json::Value,
    pub(super) modules: serde_json::Value,
    pub(super) sort: i32,
    /// 质量维度种子（0092）：kinds 标签 + dict 选项；apply 时重建，未定义的维度不动
    #[serde(default)]
    #[sqlx(default)]
    pub(super) sections: Option<serde_json::Value>,
    /// 登录页品牌区默认标语（0143）：apply 时写入 site_settings.site_tagline
    #[serde(default)]
    #[sqlx(default)]
    pub(super) tagline: String,
}
