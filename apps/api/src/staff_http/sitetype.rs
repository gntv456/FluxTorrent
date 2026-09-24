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
        "SELECT code, name, description, brand, categories, modules, \
         sort, tagline FROM site_type_packs WHERE code = $1",
    )
    .bind(&site_type)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 实际分类以 categories 表为准（类型包只是初始快照，管理组可再编辑）；
    // bg_color（0183）是分类自身的一等属性，前端不再持有硬编码色表
    let cats: Vec<(i32, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, name, icon_key, bg_color FROM categories ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 旧三列（torrents.medium_id / grade_id / edition_id）的词表：这三张实体表
    // 就是它们的 id 权威（grades 0=幼儿园…12=高三；media/editions 从 1 起），
    // 与 0088 之后新模型的 section_dict.id **不是同一套编号**（0174 回填后已是
    // 400+ 段），所以必须单独下发，前端不能再拿硬编码数组按下标补偿。
    let dict_rows = |table: &'static str| {
        let db = state.repo.db.clone();
        async move {
            sqlx::query_as::<_, (i32, String)>(
                // 表名来自本函数内的字面量常量，非用户输入
                &format!("SELECT id, name FROM {table} ORDER BY id"),
            )
            .fetch_all(&db)
            .await
            .unwrap_or_default()
        }
    };
    let grades = dict_rows("grades").await;
    let media = dict_rows("media").await;
    let editions = dict_rows("editions").await;
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
    // 字幕区口径（0146）：kind=lyric 时前端切歌词文案/格式；label 为显示名
    let (sub_kind, sub_label): (String, String) =
        sqlx::query_as::<_, (String, String)>(
            "SELECT \
             COALESCE(MAX(value) FILTER (WHERE name = 'subtitle_kind'), \
             'subtitle'), \
             COALESCE(MAX(value) FILTER (WHERE name = 'subtitle_label'), \
             '字幕') \
             FROM site_settings WHERE name IN ('subtitle_kind', \
             'subtitle_label')",
        )
        .fetch_optional(&state.repo.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| ("subtitle".into(), "字幕".into()));
    // 字幕工作流子开关（0148）：workflow=认领流程 / award=评选 /
    // ai_badge=AI 角标。前端按此显隐认领按钮/评选榜入口/AI 标记
    let sub_flags: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN \
         ('subtitle_workflow', 'subtitle_award', 'subtitle_ai_badge')",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let flag = |k: &str, d: &str| -> String {
        sub_flags
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| d.to_string())
    };
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
        "subtitle_kind": sub_kind,
        "subtitle_label": sub_label,
        "subtitle_workflow": flag("subtitle_workflow", "0") == "1",
        "subtitle_award": flag("subtitle_award", "0") == "1",
        "subtitle_ai_badge": flag("subtitle_ai_badge", "1") != "0",
        "founded": founded,
        "metadata_sources": sources,
        "site_desc": site_desc,
        "categories": cats.iter().map(|(id, name, icon, bg)| serde_json::json!({"id": id, "name": name, "icon_key": icon, "bg_color": bg})).collect::<Vec<_>>(),
        "torrent_dicts": {
            "grades": grades.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
            "media": media.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
            "editions": editions.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
        },
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
        "SELECT code, name, description, brand, categories, modules, \
         sort, tagline FROM site_type_packs ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
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
    /// 标签种子（0160 P2）：[{name, kind?, group?}]，仅站型层（scope_layer=pack）；
    /// 通用层六件套不受 apply 影响。apply 时 pack 层重建（引用保护）。
    #[serde(default)]
    #[sqlx(default)]
    pub(super) tags: Option<serde_json::Value>,
    /// 登录页品牌区默认标语（0143）：apply 时写入 site_settings.site_tagline
    #[serde(default)]
    #[sqlx(default)]
    pub(super) tagline: String,
}
