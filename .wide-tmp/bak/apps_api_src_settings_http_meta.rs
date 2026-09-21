//! 元数据行（settings_meta ⋈ site_settings）与角色/分组标签助手。
//! 从 settings_http.rs 按域拆出。

use crate::state::AppState;
use actix_web::web;

// 元数据行（settings_meta ⋈ site_settings）
// ============================================================================

#[derive(sqlx::FromRow, Clone)]
pub(super) struct MetaRow {
    pub(super) name: String,
    pub(super) kind: String,
    pub(super) label_zh: Option<String>,
    pub(super) label_en: Option<String>,
    pub(super) hint: Option<String>,
    pub(super) unit: Option<String>,
    pub(super) min: Option<f64>,
    pub(super) max: Option<f64>,
    pub(super) step: Option<f64>,
    pub(super) options: Option<serde_json::Value>,
    pub(super) secret: bool,
    pub(super) readonly: bool,
    pub(super) group_key: Option<String>,
    pub(super) min_class: Option<i32>,
    pub(super) grp: String,
    pub(super) value: String,
    pub(super) updated_at: chrono::DateTime<chrono::Utc>,
}

/// 站点设定默认可写门槛（sysop；字段级可经 settings_meta.min_class 放宽）
pub(super) const SETTINGS_WRITE_MIN: i32 = 99;
/// 单次批量保存的字段上限
pub(super) const MAX_BATCH: usize = 500;
pub(super) const META_SELECT: &str = "SELECT s.name, m.type AS kind, m.label_zh, m.label_en, m.hint, m.unit, \
        m.min, m.max, m.step, m.options, \
        COALESCE(m.secret, false) AS secret, COALESCE(m.readonly, false) AS readonly, \
        m.group_key, m.min_class, \
        COALESCE(s.grp, 'misc') AS grp, s.value, s.updated_at \
     FROM site_settings s JOIN settings_meta m ON m.name = s.name";

/// 字段可写门槛（meta.min_class 缺省 = sysop）
pub(super) fn write_min(m: &MetaRow) -> i32 {
    m.min_class.unwrap_or(SETTINGS_WRITE_MIN)
}

pub(super) fn role_of(class_id: i32) -> &'static str {
    if class_id >= 99 {
        "sysop"
    } else if class_id >= 93 {
        "administrator"
    } else {
        "moderator"
    }
}

/// 12 分区中文名（前端可再用 i18n 字典覆盖）
pub(super) fn group_label<'a>(key: &'a str) -> &'a str {
    match key {
        "basic" => "基础设定",
        "main" => "主要设定",
        "smtp" => "SMTP 设定",
        "security" => "安全设定",
        "authority" => "权限设定",
        "tweak" => "次要设定",
        "bonus" => "魔力设定",
        "account" => "账号设定",
        "torrent" => "种子设定",
        "attachment" => "附件设定",
        "advertisement" => "广告设定",
        "misc" => "其他设定",
        // U1/U5 扩展分区（0107/0109/0112 播种；未列入 DEFAULT_GROUP_ORDER 时排在末尾）
        "economy" => "经济参数",
        "anticheat" => "防作弊",
        "module_community" => "模块开关·社区",
        "module_economy" => "模块开关·经济",
        "module_fun" => "模块开关·娱乐",
        "module_ops" => "模块开关·运营",
        other => other,
    }
}

const DEFAULT_GROUP_ORDER: &[&str] = &[
    "basic",
    "main",
    "smtp",
    "security",
    "authority",
    "tweak",
    "bonus",
    "account",
    "torrent",
    "attachment",
    "advertisement",
    "anticheat",
    "economy",
    "module_community",
    "module_economy",
    "module_fun",
    "module_ops",
    "misc",
];

/// 分区顺序来自元数据键 settings_group_order；缺失则回退内置顺序
pub(super) async fn group_order(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> Vec<String> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'settings_group_order'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    let parsed: Vec<String> = raw
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parsed.is_empty() {
        DEFAULT_GROUP_ORDER.iter().map(|s| s.to_string()).collect()
    } else {
        parsed
    }
}

// ============================================================================
