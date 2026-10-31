//! site-profile 下发的分机组装（sitetype.rs 超 300 行门禁后拆出）。
//!
//! 放这里的东西都满足同一形态：**从 site_settings 读若干键 → 归一成 JSON 片段**。
//! 判据（空值算不算「未设置」、哪些格式收进来）写在函数内注释里，
//! 因为前端对这些片段是「有值才用，没值回落字典」的口径。

use serde_json::{Map, Value};
use sqlx::PgPool;

/// 读一条站点设定：NULL、空串、纯空白一律归一成 `None`（= 未设置），
/// 免得每个调用点各自写一遍 `trim + filter(non_empty)`。
pub(crate) async fn setting_text(db: &PgPool, name: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
}

/// 数值型设定（0400 作品页磨砂底图）：解析失败/缺省回落默认值，
/// 并按 [min,max] 钳制——站长手滑填 999 也不该把页面打成白屏。
pub(crate) async fn setting_num(
    db: &PgPool,
    name: &str,
    default: f64,
    min: f64,
    max: f64,
) -> f64 {
    setting_text(db, name)
        .await
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

/// 作品详情页 Hero 磨砂底图（0403）：`{blur, dim}`——海报高斯模糊半径
/// （0~80 px）与遮罩深度（0~0.9）。后台「外观与主题」分区可调，
/// clamp 在 [`setting_num`]，解析失败回落默认值。
pub(crate) async fn work_hero(db: &PgPool) -> Value {
    serde_json::json!({
        "blur": setting_num(db, "work_hero_blur", 28.0, 0.0, 80.0).await,
        "dim": setting_num(db, "work_hero_dim", 0.55, 0.0, 0.9).await,
    })
}

/// SEO（0201）：META 描述/关键词 + 是否允许被收录。
///
/// 0201 之前 metadescription 只有 RSS 消费、metakeywords 零消费，
/// 设置页上是一排「填了没用的框」（四审 L6 假开关族）。
/// `indexable` **缺省 false**：私有站被搜索引擎抓走是事故，不是特性，
/// 要放开得站长显式打开。
pub(crate) async fn seo_block(db: &PgPool) -> Value {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name IN \
         ('metadescription','metakeywords','seo_indexable')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let get = |k: &str| -> Option<String> {
        rows.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty())
    };
    let mut seo = Map::new();
    if let Some(v) = get("metadescription") {
        seo.insert("description".into(), Value::String(v));
    }
    if let Some(v) = get("metakeywords") {
        seo.insert("keywords".into(), Value::String(v));
    }
    seo.insert(
        "indexable".into(),
        Value::Bool(get("seo_indexable").as_deref() == Some("yes")),
    );
    Value::Object(seo)
}

/// 术语表（0205 / 四审 L7）：`[{canonical, replacement}]`，只下发启用中的规则。
///
/// 前端 `apply-terms.ts` 拿它在字典出口做一次改写；后端错误信封走
/// `crate::terms::apply` 同一份规则。**零规则下发空数组**，前端据此走恒等快速路径
/// （不改写、不重新分配）。
pub(crate) async fn terms(db: &PgPool) -> Value {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT canonical, replacement FROM site_terms \
         WHERE enabled ORDER BY length(canonical) DESC, sort, canonical \
         LIMIT $1",
    )
    .bind(crate::terms::MAX_RULES)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    Value::Array(
        rows.into_iter()
            .map(|(canonical, replacement)| {
                serde_json::json!({
                    "canonical": canonical,
                    "replacement": replacement,
                })
            })
            .collect(),
    )
}

/// 站点 / 登录页背景图（0445）：URL 空 = 不启用；不透明度 0~100。
pub(crate) async fn backgrounds(db: &PgPool) -> Value {
    serde_json::json!({
        "site": {
            "url": setting_text(db, "site_bg_url").await,
            "opacity": setting_num(db, "site_bg_opacity", 25.0, 0.0, 100.0)
                .await,
        },
        "login": {
            "url": setting_text(db, "login_bg_url").await,
            "opacity": setting_num(db, "login_bg_opacity", 50.0, 0.0, 100.0)
                .await,
        },
        "loginPage": {
            "url": setting_text(db, "login_page_bg_url").await,
            "opacity": setting_num(
                db, "login_page_bg_opacity", 40.0, 0.0, 100.0,
            )
            .await,
        },
    })
}

/// 主题令牌（0189 R4.6）：有值才下发，前端注入 `:root` 覆盖默认色。
/// 只收 `#rrggbb`——这是站长能写进 CSS 的唯一入口，放开了就是注入面。
pub(crate) async fn theme_tokens(db: &PgPool) -> Value {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings \
         WHERE name LIKE 'theme_token_%'",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let mut out = Map::new();
    for (name, value) in rows {
        let v = value.trim();
        let b = v.as_bytes();
        if b.len() == 7
            && b[0] == b'#'
            && b[1..].iter().all(|c| c.is_ascii_hexdigit())
        {
            out.insert(name, Value::String(v.to_string()));
        }
    }
    Value::Object(out)
}
