//! 站点资料与站型包列表。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};

use super::profile_bits;
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
    let db = &state.repo.db;
    let site_type: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_type'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "general".into());
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, \
         sort, tagline, subtitle_kind FROM site_type_packs WHERE code = $1",
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
    // 就是它们的 id 权威，与 0088 之后新模型的 section_dict.id **不是同一套编号**，
    // 所以下发空表时前端下拉自然消失（0180：非教育站的学段/版本词表已清空）。
    // 教育系站型（education/ebook）仍下发全量；其余站型（含 custom_*，0209 修正：
    // 自定义站型是「不偏向任何 PT 类型」的最后防线，不再强灌学段/版本词）为空数组。
    let edu_like = matches!(site_type.as_str(), "education" | "ebook");
    let dict_rows = |table: &'static str, emit: bool| {
        let db = state.repo.db.clone();
        async move {
            if !emit {
                return Vec::new();
            }
            sqlx::query_as::<_, (i32, String)>(
                // 表名来自本函数内的字面量常量，非用户输入
                &format!("SELECT id, name FROM {table} ORDER BY id"),
            )
            .fetch_all(&db)
            .await
            .unwrap_or_default()
        }
    };
    let grades = dict_rows("grades", edu_like).await;
    let media = dict_rows("media", true).await;
    let editions = dict_rows("editions", edu_like).await;
    let brand: String = sqlx::query_scalar(
        // 键收敛（0214 P2-4.6）：site_name 是权威键（向导/后台主要写它）；
        // SITENAME 是 NexusPHP 口径的旧键（RSS 频道名在用），仅作回落，不再各自为政
        "SELECT COALESCE(NULLIF((SELECT value FROM site_settings WHERE name \
         = 'site_name'), ''), \
         (SELECT value FROM site_settings WHERE name = 'SITENAME'), '')",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .flatten()
    .or(pack.as_ref().map(|p| p.brand.clone()))
    .unwrap_or_default();
    // 站点货币名（0082）：默认「魔力」，站长可后台改任意名；空值兜底回默认
    let currency: String = profile_bits::setting_text(db, "currency_name")
        .await
        .unwrap_or_else(|| "魔力".to_string());
    // 建站日期（页脚版权条 "(c) 站名 日期 Powered by FluxTorrent" 用）
    let founded: Option<String> =
        profile_bits::setting_text(db, "datefounded").await;
    // 默认语言（0216 G1 真驱动）：NP 口径 en|chs|cht（与 users.site_language 同码制），
    // 前端 getLocale 对「无语言 cookie 的新访客」回落到它，再映射到 BCP47 文案
    let default_language: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'default_language'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "chs".into());

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
        .unwrap_or_else(|| "imdb,douban,bangumi,indienova,mediainfo".into())
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
    let site_logo = profile_bits::setting_text(db, "site_logo").await;
    // 站点图标（0214）：浏览器标签页 favicon；空 = 前端用内置 icon
    let site_favicon = profile_bits::setting_text(db, "site_favicon").await;
    // 站点简介（0088）：页脚「站点信息」卡片文案，留空由前端回落字典默认
    let site_desc = profile_bits::setting_text(db, "site_desc").await;
    // SEO（0201）与主题令牌（0189）的判据在 profile_bits（空值/格式口径）
    let seo = profile_bits::seo_block(db).await;
    // 主题令牌（0189 R4.6）：有值才下发——前端注入 :root 覆盖默认 Aurora 色
    let theme_tokens = profile_bits::theme_tokens(db).await;
    // 术语表（0205 四审 L7）：前端字典出口改写的规则源
    let terms = profile_bits::terms(db).await;
    Ok(ok(serde_json::json!({
        "theme_tokens": theme_tokens,
        "terms": terms,
        "site_type": site_type,
        "pack_name": pack.as_ref().map(|p| p.name.clone()),
        "brand": brand,
        "tagline": tagline,
        "site_logo": site_logo,
        "site_favicon": site_favicon,
        "currency_name": currency,
        "subtitle_kind": sub_kind,
        "subtitle_label": sub_label,
        // 语言切换器显隐（0209 P2-17）：no = 隐藏（单语站）
        "locale_switcher_enabled": profile_bits::setting_text(
            db,
            "locale_switcher_enabled",
        )
        .await
        .unwrap_or_else(|| "yes".into()),
        "default_language": default_language,
        "subtitle_workflow": flag("subtitle_workflow", "0") == "1",
        "subtitle_award": flag("subtitle_award", "0") == "1",
        "subtitle_ai_badge": flag("subtitle_ai_badge", "1") != "0",
        "founded": founded,
        "metadata_sources": sources,
        "site_desc": site_desc,
        "seo": seo,
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
         sort, tagline, subtitle_kind FROM site_type_packs ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(crate) struct SiteTypePack {
    pub(crate) code: String,
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
    /// 字幕区口径快照（0178）：lyric/subtitle；NULL = 未表态（apply 按 code 回落）
    #[serde(default)]
    #[sqlx(default)]
    pub(super) subtitle_kind: Option<String>,
    /// 术语规则段（0206）：`[{canonical,replacement,sort}]`；
    /// **NULL = 本包不声明术语**（内置预置包全为 NULL，切站型不动词汇表）
    #[serde(default)]
    #[sqlx(default)]
    pub(super) terms: Option<serde_json::Value>,
}
