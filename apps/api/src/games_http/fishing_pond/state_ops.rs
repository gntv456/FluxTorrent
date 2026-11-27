//! 钓场状态·订单/好友/赛季/成就操作（2026-11-28 从 state.rs 拆出守行数门）。
//! 全部操作 state JSON 壳（default_state/hydrate 建立的形状），
//! 经 `use super::state::*` 取时间与取值工具；fnv1a/mulberry32
//! 哈希/PRNG 仅本件使用。
use serde_json::{json, Value};

use super::data::data;
use super::state::{
    active_set, day_number, gear_owned_count, n, now_ms, s, season_base_day,
    today_key,
};

/* ================= 订单 / 签到 / 好友 / 赛季 ================= */

pub(super) fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(16_777_619);
    }
    h
}

pub(super) fn mulberry32(mut a: u32) -> impl FnMut() -> f64 {
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
