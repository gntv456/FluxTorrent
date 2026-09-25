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
