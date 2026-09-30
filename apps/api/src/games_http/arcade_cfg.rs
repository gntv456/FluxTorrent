//! 娱乐屋大厅游戏表面的静态配置（货架品类 / 门禁行构造器）。
//!
//! 周常与赛季里程碑**不再住在这里**：0247 把它们落成 `arcade_quests` /
//! `arcade_milestones` 两张行表，站长在后台改得动，读取统一走
//! `arcade_rewards.rs`。代码里只留机制，参数与内容进表 —— 留一份常量就是
//! 第二份清单，界面改了玩法照旧的那种假配置就是这么来的。

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
