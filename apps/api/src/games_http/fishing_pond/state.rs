//! 甜梦钓场：存档形状 / 时间口径 / 属性汇总 / 背包与各类派生视图。
//!
//! 存档是**整份 JSON**（与原客户端 `engine.defaultState()` 同形状），字段一律小驼峰。
//! 数值口径逐条对齐 `design/.../game/js/engine.js`，两边任何一边改动都要同步。

use chrono::{FixedOffset, TimeZone, Utc};
use serde_json::{json, Value};

use super::data::data;

pub const DAY_MS: i64 = 86_400_000;
pub const TZ_OFFSET_MS: i64 = 8 * 3_600_000;

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn tz() -> FixedOffset {
    FixedOffset::east_opt((TZ_OFFSET_MS / 3_600_000) as i32).expect("+8 时区")
}

/// 用户本地（UTC+8）日期键，与客户端 todayKey 同口径。
pub fn today_key(now: i64) -> String {
    let dt = Utc
        .timestamp_millis_opt(now)
        .single()
        .unwrap_or_else(Utc::now)
        .with_timezone(&tz());
    dt.format("%Y-%m-%d").to_string()
}

/// 以 UTC+8 为边界的“天序号”，用于赛季与签到周期。
pub fn day_number(now: i64) -> i64 {
    (now + TZ_OFFSET_MS).div_euclid(DAY_MS)
}

pub fn season_base_day() -> i64 {
    let base = tz()
        .with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .expect("2026-01-01");
    day_number(base.timestamp_millis())
}

/* ================= 取值助手 ================= */

pub fn n(v: &Value, k: &str) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(0)
}
pub fn f(v: &Value, k: &str) -> f64 {
    v.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}
pub fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
pub fn round2(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}

/* ================= 默认存档 / 补全 ================= */

pub fn default_state(now: i64) -> Value {
    let d = data();
    let mut equipped = serde_json::Map::new();
    let mut owned = serde_json::Map::new();
    for cat in ["rod", "bait", "hook", "line", "float"] {
        if let Some(it) = d.default_item(cat) {
            equipped.insert(cat.to_string(), json!(it.id));
            owned.insert(
                cat.to_string(),
                json!([{ "id": it.id, "lv": 1, "at": now }]),
            );
        }
    }
    // stats 单独一个 json!：顶层宏键数已贴着 serde_json 默认递归深度。
    let stats = json!({
        "casts": 0, "catches": 0, "breaks": 0, "missed": 0, "plays": 0,
        "magicEarned": 0, "rareCaught": 0, "epicCaught": 0, "legendCaught": 0,
        "nightCatches": 0, "rainCatches": 0, "bigCaught": 0, "bestStreak": 0,
        "curStreak": 0, "ordersDone": 0, "giftsSent": 0, "signFull": 0, "released": 0
    });
    json!({
        "v": 1,
        // 魔力=站点余额镜像，起手 0（不送魔力；读档时 spark_of 覆盖）。
        "magic": 0, "freeCasts": 3, "baitId": "worm", "baits": {}, "luck": 0,
        "stamina": { "v": d.k.stamina_max, "at": now },
        "scene": "bay",
        "cond": "day",
        "scenesOwned": ["bay"],
        "equipped": equipped,
        "owned": owned,
        "autoUnlocked": false,
        "auto": false,
        "bag": { "tier": 0, "fish": [] },
        "aquarium": [],
        "album": {},
        "orders": { "day": "", "list": [] },
        "sign": { "idx": 0, "last": "", "cycle": 0 },
        "friends": { "day": "", "sent": 0, "received": [] },
        "season": { "id": 0, "catches": 0, "claimed": false },
        "ach": {},
        "stats": stats,
        "settings": { "reduceMotion": false },
        "createdAt": now
    })
}

/// 老存档补字段：只补缺，不覆盖玩家已有值。
pub fn hydrate(mut st: Value, now: i64) -> Value {
    if !st.is_object() {
        return default_state(now);
    }
    let base = default_state(now);
    for (k, v) in base.as_object().expect("default obj").clone() {
        if st.get(&k).is_none() {
            st[&k] = v;
        }
    }
    let mut stats = st.get("stats").cloned().unwrap_or_else(|| json!({}));
    if !stats.is_object() {
        stats = json!({});
    }
    for (k, v) in base["stats"].as_object().expect("stats").clone() {
        if stats.get(&k).is_none() {
            stats[&k] = v;
        }
    }
    st["stats"] = stats;
    if !st["settings"].is_object() {
        st["settings"] = base["settings"].clone();
    }
    if !st["bag"].is_object() {
        st["bag"] = base["bag"].clone();
    }
    for key in ["scenesOwned", "aquarium"] {
        if !st[key].is_array() {
            st[key] = json!([]);
        }
    }
    if !st["scenesOwned"]
        .as_array()
        .map_or(false, |a| !a.is_empty())
    {
        st["scenesOwned"] = json!(["bay"]);
    }
    ensure_equipped(&mut st, now);
    // 场景 / 时段越界兜底
    let scenes: Vec<String> = st["scenesOwned"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let sc = s(&st, "scene").to_string();
    if !scenes.iter().any(|x| x == &sc) {
        st["scene"] =
            json!(scenes.first().cloned().unwrap_or_else(|| "bay".into()));
    }
    let scene_id = s(&st, "scene").to_string();
    if let Some(scene) = data().scene(&scene_id) {
        let cond = s(&st, "cond").to_string();
        if !scene.conds.iter().any(|c| c == &cond) {
            st["cond"] = json!(scene
                .conds
                .first()
                .cloned()
                .unwrap_or_else(|| "day".into()));
        }
    }
    st
}

/* ================= 体力 ================= */

pub fn recover_stamina(st: &mut Value, now: i64) -> i64 {
    let d = data();
    let max = d.k.stamina_max;
    let mut v = st["stamina"]["v"].as_i64().unwrap_or(max);
    let at = st["stamina"]["at"].as_i64().unwrap_or(now);
    if v >= max || at > now {
        st["stamina"]["at"] = json!(now);
        return 0;
    }
    let gained = (now - at) / d.k.stamina_recover_ms;
    if gained <= 0 {
        return 0;
    }
    let before = v;
    v = (v + gained).min(max);
    st["stamina"]["v"] = json!(v);
    st["stamina"]["at"] = json!(if v >= max {
        now
    } else {
        at + gained * d.k.stamina_recover_ms
    });
    v - before
}

/* ================= 装备属性 ================= */

#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub window: f64,
    pub bite_speed: f64,
    pub hook_rate: f64,
    pub escape: f64,
    pub rare: f64,
    pub legend: f64,
    pub limit: f64,
    pub alert: f64,
    pub pull: f64,
    pub big_fish: f64,
    pub night: f64,
    pub common: f64,
    pub all: f64,
    pub set_id: Option<String>,
    pub set_bonus: Vec<Value>,
}

pub fn owned_entry<'a>(
    st: &'a Value,
    cat: &str,
    id: &str,
) -> Option<&'a Value> {
    st["owned"][cat]
        .as_array()?
        .iter()
        .find(|x| s(x, "id") == id)
}

pub fn is_owned(st: &Value, cat: &str, id: &str) -> bool {
    owned_entry(st, cat, id).is_some()
}

pub fn item_lv(st: &Value, cat: &str, id: &str) -> i64 {
    owned_entry(st, cat, id)
        .map(|x| n(x, "lv").max(1))
        .unwrap_or(0)
}

/// 装备槽为空 / 已下架 / 未持有时补默认款（存档兼容与数据升级兜底）。
pub fn ensure_equipped(st: &mut Value, now: i64) {
    for cat in ["rod", "bait", "hook", "line", "float"] {
        let id = s(&st["equipped"], cat).to_string();
        if id.is_empty() || !is_owned(st, cat, &id) {
            let Some(def) = data().default_item(cat) else {
                continue;
            };
            st["equipped"][cat] = json!(def.id);
            if !is_owned(st, cat, &def.id) {
                let entry = json!({"id": def.id, "lv": 1, "at": now});
                match st["owned"][cat].as_array_mut() {
                    Some(a) => a.push(entry),
                    None => st["owned"][cat] = json!([entry]),
                }
            }
        }
    }
}

pub fn active_set(st: &Value) -> Option<&'static super::data::Set> {
    let scene = s(st, "scene");
    for set in &data().sets {
        if set.scene != scene {
            continue;
        }
        let all = set
            .parts
            .iter()
            .all(|(cat, id)| s(&st["equipped"], cat) == id);
        if all {
            return Some(set);
        }
    }
    None
}

pub fn stats_of(st: &Value) -> Stats {
    let d = data();
    let mut out = Stats {
        window: 800.0,
        limit: 100.0,
        pull: 1.0,
        ..Default::default()
    };
    for cat in ["rod", "bait", "hook", "line", "float"] {
        let id = s(&st["equipped"], cat).to_string();
        let Some(it) = d.item(cat, &id) else { continue };
        let lv = item_lv(st, cat, &id).max(1);
        let bonus = (lv - 1) as f64 / (d.k.upgrade_max_lv - 1).max(1) as f64;
        for (k, v) in &it.eff {
            let factor = match k.as_str() {
                "window" => 0.35,
                "limit" => 0.4,
                "bigFish" | "night" => 0.2,
                _ => 0.3,
            };
            let scaled = v * (1.0 + bonus * factor);
            match k.as_str() {
                "window" => out.window += scaled,
                "biteSpeed" => out.bite_speed += scaled,
                "hookRate" => out.hook_rate += scaled,
                "escape" => out.escape += scaled,
                "rare" => out.rare += scaled,
                "legend" => out.legend += scaled,
                "limit" => out.limit += scaled,
                "alert" => out.alert += scaled,
                "pull" => out.pull += scaled,
                "bigFish" => out.big_fish += scaled,
                "night" => out.night += scaled,
                "common" => out.common += scaled,
                _ => {}
            }
        }
    }
    if let Some(set) = active_set(st) {
        out.set_id = Some(set.id.clone());
        for b in &set.bonuses {
            match b.k.as_str() {
                "all" => out.all += b.v,
                "nightRare" | "deepFish" => out.rare += b.v,
                "biteSpeed" => out.bite_speed += b.v,
                "hookRate" => out.hook_rate += b.v,
                "escape" => out.escape += b.v,
                "rare" => out.rare += b.v,
                "legend" => out.legend += b.v,
                "limit" => out.limit += b.v,
                "alert" => out.alert += b.v,
                "pull" => out.pull += b.v,
                "bigFish" => out.big_fish += b.v,
                "night" => out.night += b.v,
                "common" => out.common += b.v,
                _ => {}
            }
            out.set_bonus.push(json!({"k": b.k, "v": b.v}));
        }
    }
    out.window = (out.window * (1.0 + out.alert + out.all)).round();
    out.pull = round2(out.pull + out.all);
    out.limit = round2(out.limit + out.all * 40.0);
    out.bite_speed = round2(out.bite_speed + out.all);
    out
}

impl Stats {
    pub fn to_json(&self) -> Value {
        json!({
            "window": self.window,
            "biteSpeed": round2(self.bite_speed),
            "hookRate": round2(self.hook_rate),
            "escape": round2(self.escape),
            "rare": round2(self.rare),
            "legend": round2(self.legend),
            "limit": self.limit,
            "alert": round2(self.alert),
            "pull": self.pull,
            "bigFish": round2(self.big_fish),
            "night": round2(self.night),
            "common": round2(self.common),
            "all": round2(self.all),
            "setId": self.set_id,
            "setBonus": self.set_bonus,
        })
    }
}

/* ================= 背包 / 鱼缸 ================= */

pub fn gear_owned_count(st: &Value) -> i64 {
    st["owned"]
        .as_object()
        .map(|m| {
            m.values()
                .map(|v| v.as_array().map_or(0, |a| a.len() as i64))
                .sum()
        })
        .unwrap_or(0)
}

pub fn bag_cap(st: &Value) -> i64 {
    let tier = n(&st["bag"], "tier") as usize;
    data().k.bag_tiers.get(tier).map(|t| t.cap).unwrap_or(30)
}

pub fn bag_used(st: &Value) -> i64 {
    st["bag"]["fish"].as_array().map_or(0, |a| a.len() as i64)
        + gear_owned_count(st)
}

pub fn bag_free(st: &Value) -> i64 {
    (bag_cap(st) - bag_used(st)).max(0)
}

pub fn aquarium_cap(st: &Value) -> i64 {
    (n(&st["bag"], "tier") + 1) * data().k.aquarium_cap_per_tier
}

pub fn place_catch(st: &mut Value, entry: Value) -> &'static str {
    if bag_free(st) > 0 {
        if let Some(arr) = st["bag"]["fish"].as_array_mut() {
            arr.push(entry);
        }
        "bag"
    } else if st["aquarium"].as_array().map_or(0, |a| a.len())
        < aquarium_cap(st) as usize
    {
        if let Some(arr) = st["aquarium"].as_array_mut() {
            arr.push(entry);
        }
        "aquarium"
    } else {
        "lost"
    }
}

/* ================= 订单 / 签到 / 好友 / 赛季 ================= */

fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16_777_619);
    }
    h
}

fn mulberry32(mut a: u32) -> impl FnMut() -> f64 {
    move || {
        a = a.wrapping_add(0x6d2b_79f5);
        let mut t = a;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64) / 4_294_967_296.0
    }
}

pub fn orders_ensure(st: &mut Value, now: i64) {
    let key = today_key(now);
    if s(&st["orders"], "day") == key
        && st["orders"]["list"]
            .as_array()
            .map_or(false, |a| !a.is_empty())
    {
        return;
    }
    let mut r = mulberry32(fnv1a(&format!("{}|{}", key, s(st, "scene"))));
    let scenes: Vec<String> = st["scenesOwned"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let mut pool: Vec<&super::data::Fish> = data()
        .fish
        .iter()
        .filter(|x| scenes.iter().any(|s| s == &x.scene))
        .collect();
    if pool.len() < 3 {
        pool = data().fish.iter().collect();
    }
    let mut list = Vec::new();
    let mut used: Vec<String> = Vec::new();
    for _ in 0..3 {
        let mut idx = (r() * pool.len() as f64).floor() as usize;
        let mut guard = 0;
        while used.iter().any(|u| u == &pool[idx].id) && guard < 30 {
            idx = (r() * pool.len() as f64).floor() as usize;
            guard += 1;
        }
        let f = pool[idx];
        used.push(f.id.clone());
        let need = 1 + (r() * 3.0).floor() as i64;
        list.push(json!({
            "id": f.id, "need": need, "got": 0,
            "reward": (f.value as f64 * 2.6 * need as f64).round() as i64,
            "claimed": false
        }));
    }
    st["orders"] = json!({ "day": key, "list": list });
}

pub fn bump_orders(st: &mut Value, fish_id: &str) -> i64 {
    let mut hits = 0;
    if let Some(list) = st["orders"]["list"].as_array_mut() {
        for o in list {
            if s(o, "id") == fish_id
                && !o["claimed"].as_bool().unwrap_or(false)
                && n(o, "got") < n(o, "need")
            {
                o["got"] = json!(n(o, "got") + 1);
                hits += 1;
            }
        }
    }
    hits
}

/// 好友园：每天每个好友寄 1 份鱼饵，收信上限 `giftReceiveCap`。
pub fn friends_ensure(st: &mut Value, now: i64) {
    let key = today_key(now);
    if s(&st["friends"], "day") == key {
        return;
    }
    let cap = data().k.gift_receive_cap as usize;
    let inbox: Vec<Value> = data()
        .friends
        .iter()
        .take(cap)
        .map(|f| json!(f.id))
        .collect();
    st["friends"] = json!({ "day": key, "sent": 0, "received": inbox });
}

pub fn friends_gift(st: &mut Value) -> Result<i64, &'static str> {
    let limit = data().k.gift_daily_limit;
    let sent = n(&st["friends"], "sent");
    if sent >= limit {
        return Err("limit");
    }
    st["friends"]["sent"] = json!(sent + 1);
    st["stats"]["giftsSent"] = json!(n(&st["stats"], "giftsSent") + 1);
    Ok(limit - sent - 1)
}

pub fn friends_receive(st: &mut Value) -> Result<i64, &'static str> {
    let got = st["friends"]["received"]
        .as_array()
        .map_or(0, |a| a.len())
        .min(data().k.gift_receive_cap as usize);
    if got == 0 {
        return Err("empty");
    }
    if let Some(arr) = st["friends"]["received"].as_array_mut() {
        arr.drain(0..got);
    }
    st["freeCasts"] = json!(n(st, "freeCasts") + got as i64);
    Ok(got as i64)
}

pub fn season_info(now: i64) -> (i64, i64, i64) {
    let day = day_number(now);
    let base = season_base_day();
    let days = data().k.season_days.max(1);
    let id = (day - base).div_euclid(days);
    let day_idx = (day - base).rem_euclid(days);
    (id, day_idx, days - day_idx)
}

pub fn season_ensure(st: &mut Value, now: i64) {
    let (id, _, _) = season_info(now);
    if n(&st["season"], "id") != id || st["season"].is_null() {
        st["season"] = json!({ "id": id, "catches": 0, "claimed": false });
    }
}

/* ================= 成就 ================= */

pub fn metric_value(st: &Value, metric: &str) -> i64 {
    match metric {
        "albumCount" => st["album"].as_object().map_or(0, |m| m.len() as i64),
        "sceneCount" => {
            st["scenesOwned"].as_array().map_or(0, |a| a.len() as i64)
        }
        "setActive" => i64::from(active_set(st).is_some()),
        "gearOwned" => gear_owned_count(st),
        "maxLv" => st["owned"]
            .as_object()
            .map(|m| {
                m.values()
                    .filter_map(Value::as_array)
                    .flat_map(|a| a.iter())
                    .map(|x| n(x, "lv").max(1))
                    .max()
                    .unwrap_or(1)
            })
            .unwrap_or(1),
        "bagTier" => n(&st["bag"], "tier"),
        _ => n(&st["stats"], metric),
    }
}

pub fn ach_claimable(
    st: &Value,
    id: &str,
) -> Option<&'static super::data::Ach> {
    let a = data().ach.iter().find(|a| a.id == id)?;
    let claimed = st["ach"][id]["claimed"].as_bool().unwrap_or(false);
    if claimed || metric_value(st, &a.metric) < a.target {
        return None;
    }
    Some(a)
}
