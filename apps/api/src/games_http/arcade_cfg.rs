//! 娱乐屋大厅游戏表面的内容定义（周常 / 赛季里程碑 / 货架品类）。
//! code 稳定、文案走前端 i18n；与 games.rs 的赔率常量同口径 —— 单一真相源在
//! 代码（不落库），改前先看这里的注释与门禁。

/// 周常：code, 归属玩法(ref_type；"*" = 任意), 目标局数, 奖励魔力。
/// 奖励为魔力（非经济类物品）：确定侧发放构造上不进奖池侧门。
pub(super) const QUESTS: [(&str, &str, i64, i64); 4] = [
    ("q_scratch", "scratch", 12, 100),
    ("q_bs", "bigsmall", 10, 150),
    ("q_farm", "farm_water", 3, 100),
    ("q_week", "*", 30, 200),
];

/// 当前赛季 key（里程碑按集齐票根数解锁）
pub(super) const SEASON_KEY: &str = "S1";

/// 赛季里程碑：code, 需集齐票根数, 奖励魔力
pub(super) const MILESTONES: [(&str, i64, i64); 4] = [
    ("m1", 3, 300),
    ("m2", 6, 600),
    ("m3", 9, 900),
    ("m4", 12, 1500),
];

/// 零负债（纯外观，无经济产出）：外观货架只列这些品类
pub(super) const COSMETIC_KINDS: [&str; 8] = [
    "avatar_frame",
    "animated_avatar",
    "rainbow_name",
    "rainbow_id",
    "custom_title",
    "makeup_card",
    "rename_card",
    "ad_free",
];

/// 门禁条目（与前端 GateBadge 同构）
pub(super) fn bad(name: &str, pass: bool, why: String) -> serde_json::Value {
    serde_json::json!({ "name": name, "pass": pass, "why": why })
}
