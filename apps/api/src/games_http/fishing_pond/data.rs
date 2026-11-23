//! 甜梦钓场（fishing pond）：静态配置。
//!
//! 数据源是设计目录里的 `design/.../game/js/data.js`，由
//! `design/arcade-sweet-dream/game/tools/gen-server-data.mjs` 导出成同目录 `data.json`。
//! 改数值只改 `data.js` 再重跑脚本，别在这里手改 —— 客户端与服务端必须同一套表。

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::Deserialize;

const RAW: &str = include_str!("data.json");

static DATA: OnceLock<Data> = OnceLock::new();

pub fn data() -> &'static Data {
    DATA.get_or_init(|| {
        serde_json::from_str(RAW).expect("fishing_pond/data.json 解析失败：先用 gen-server-data.mjs 重新生成")
    })
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub rarity: HashMap<String, Rarity>,
    pub rarity_order: Vec<String>,
    pub scenes: Vec<Scene>,
    pub items: HashMap<String, Vec<Item>>,
    pub sets: Vec<Set>,
    pub fish: Vec<Fish>,
    #[serde(rename = "const")]
    pub k: Consts,
    pub ach: Vec<Ach>,
    pub friends: Vec<Friend>,
    /// 单机/NPC 模式的对手表；服务端赛季榜走真实玩家，不读这份配置。
    #[allow(dead_code)]
    pub rivals: Vec<Rival>,
    pub rank_rewards: Vec<i64>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Rarity {
    pub weight: f64,
    pub strength: f64,
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub conds: Vec<String>,
    pub price: i64,
}

#[derive(Deserialize, Debug)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub price: i64,
    #[serde(default)]
    pub eff: HashMap<String, f64>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Set {
    pub id: String,
    pub name: String,
    pub scene: String,
    pub parts: HashMap<String, String>,
    pub bonuses: Vec<Bonus>,
}

#[derive(Deserialize, Debug)]
pub struct Bonus {
    pub k: String,
    pub v: f64,
}

#[derive(Deserialize, Debug)]
pub struct Fish {
    pub id: String,
    pub name: String,
    pub scene: String,
    pub cond: String,
    pub rarity: String,
    pub value: i64,
    pub size: [f64; 2],
    #[serde(default = "one")]
    pub river: f64,
    #[serde(default)]
    pub need_float: Option<String>,
}

fn one() -> f64 {
    1.0
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Consts {
    pub stamina_max: i64,
    pub stamina_recover_ms: i64,
    pub stamina_refill_cost: i64,
    pub cast_stamina_cost: i64,
    pub bag_tiers: Vec<BagTier>,
    pub aquarium_cap_per_tier: i64,
    pub auto_unlock_price: i64,
    pub order_all_bonus: i64,
    pub gift_daily_limit: i64,
    pub gift_receive_cap: i64,
    pub season_days: i64,
    pub sign_in_rewards: Vec<SignReward>,
    pub upgrade_max_lv: i64,
    pub upgrade_cost_rate: f64,
}

#[derive(Deserialize, Debug)]
pub struct BagTier {
    pub cap: i64,
    pub price: i64,
}

#[derive(Deserialize, Debug)]
pub struct SignReward {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: i64,
    pub label: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Ach {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub target: i64,
    pub reward: i64,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Friend {
    pub id: String,
    pub name: String,
    pub seed: i64,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Rival {
    pub id: String,
    pub name: String,
    pub base: i64,
    pub growth: f64,
}

impl Data {
    pub fn fish(&self, id: &str) -> Option<&Fish> {
        self.fish.iter().find(|f| f.id == id)
    }
    pub fn item(&self, cat: &str, id: &str) -> Option<&Item> {
        self.items.get(cat)?.iter().find(|i| i.id == id)
    }
    /// 装机默认款：该分类里 price == 0 的第一件（与 engine.ensureEquipped 同口径）
    pub fn default_item(&self, cat: &str) -> Option<&Item> {
        let list = self.items.get(cat)?;
        list.iter().find(|i| i.price == 0).or_else(|| list.first())
    }
    pub fn scene(&self, id: &str) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == id)
    }
    pub fn scene_index(&self, id: &str) -> Option<usize> {
        self.scenes.iter().position(|s| s.id == id)
    }
}
