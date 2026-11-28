//! 钓场·商店/装备/体力规则（2026-11-28 从 rules.rs 拆出守行数门禁）：
//! 买道具/穿装备/升级（升级价 = 原价 × 等级）/买场景/切场景/设条件/
//! 扩背包/解锁自动钓/补体力。操作 state JSON 壳（default_state/hydrate
//! 建立的形状），工具经 `use super::state::*`。
use serde_json::{json, Value};

use super::data::data;
use super::state::*;

pub fn buy_item(
    st: &mut Value,
    cat: &str,
    id: &str,
    now: i64,
) -> Result<Value, &'static str> {
    let Some(it) = data().item(cat, id) else {
        return Err("notfound");
    };
    if is_owned(st, cat, id) {
        return Err("owned");
    }
    let price = it.price;
    if n(st, "magic") < price {
        return Err("poor");
    }
    st["magic"] = json!(n(st, "magic") - price);
    let entry = json!({"id": id, "lv": 1, "at": now});
    match st["owned"][cat].as_array_mut() {
        Some(a) => a.push(entry),
        None => st["owned"][cat] = json!([entry]),
    }
    Ok(
        json!({"ok": true, "cost": price, "item": {"id": it.id, "name": it.name}}),
    )
}

pub fn equip_item(
    st: &mut Value,
    cat: &str,
    id: &str,
) -> Result<Value, &'static str> {
    if !is_owned(st, cat, id) {
        return Err("notowned");
    }
    st["equipped"][cat] = json!(id);
    Ok(json!({"ok": true, "setId": active_set(st).map(|s| s.id.clone())}))
}

pub fn upgrade_cost(price: i64, lv: i64) -> i64 {
    (price.max(120) as f64 * data().k.upgrade_cost_rate * lv as f64).round()
        as i64
}

pub fn upgrade_item(
    st: &mut Value,
    cat: &str,
    id: &str,
) -> Result<Value, &'static str> {
    let Some(it) = data().item(cat, id) else {
        return Err("notowned");
    };
    let Some(entry) = owned_entry(st, cat, id) else {
        return Err("notowned");
    };
    let lv = n(entry, "lv").max(1);
    if lv >= data().k.upgrade_max_lv {
        return Err("max");
    }
    let cost = upgrade_cost(it.price, lv);
    if n(st, "magic") < cost {
        return Err("poor");
    }
    st["magic"] = json!(n(st, "magic") - cost);
    if let Some(arr) = st["owned"][cat].as_array_mut() {
        if let Some(e) = arr.iter_mut().find(|x| s(x, "id") == id) {
            e["lv"] = json!(lv + 1);
        }
    }
    Ok(json!({"ok": true, "lv": lv + 1, "cost": cost}))
}

pub fn buy_scene(st: &mut Value, id: &str) -> Result<Value, &'static str> {
    let Some(sc) = data().scene(id) else {
        return Err("notfound");
    };
    if st["scenesOwned"]
        .as_array()
        .map_or(false, |a| a.iter().any(|x| x.as_str() == Some(id)))
    {
        return Err("owned");
    }
    let idx = data().scene_index(id).unwrap_or(0);
    if idx > 0 {
        let prev = &data().scenes[idx - 1];
        if !st["scenesOwned"].as_array().map_or(false, |a| {
            a.iter().any(|x| x.as_str() == Some(prev.id.as_str()))
        }) {
            return Err("locked");
        }
    }
    if sc.price > 0 && n(st, "magic") < sc.price {
        return Err("poor");
    }
    if sc.price > 0 {
        st["magic"] = json!(n(st, "magic") - sc.price);
    }
    if let Some(arr) = st["scenesOwned"].as_array_mut() {
        arr.push(json!(id));
    }
    Ok(json!({"ok": true, "cost": sc.price, "scene": sc.name}))
}

pub fn switch_scene(st: &mut Value, id: &str) -> Result<Value, &'static str> {
    let Some(sc) = data().scene(id) else {
        return Err("notfound");
    };
    if !st["scenesOwned"]
        .as_array()
        .map_or(false, |a| a.iter().any(|x| x.as_str() == Some(id)))
    {
        return Err("locked");
    }
    st["scene"] = json!(id);
    let cond = s(st, "cond").to_string();
    if !sc.conds.iter().any(|c| c == &cond) {
        st["cond"] =
            json!(sc.conds.first().cloned().unwrap_or_else(|| "day".into()));
    }
    Ok(json!({"ok": true, "scene": sc.name}))
}

pub fn set_cond(st: &mut Value, cond: &str) -> Result<Value, &'static str> {
    let scene_id = s(st, "scene").to_string();
    let Some(sc) = data().scene(&scene_id) else {
        return Err("notfound");
    };
    if !sc.conds.iter().any(|c| c == cond) {
        return Err("cond");
    }
    st["cond"] = json!(cond);
    Ok(json!({"ok": true, "cond": cond}))
}

pub fn expand_bag(st: &mut Value) -> Result<Value, &'static str> {
    let tier = n(&st["bag"], "tier") as usize;
    let Some(next) = data().k.bag_tiers.get(tier + 1) else {
        return Err("max");
    };
    if n(st, "magic") < next.price {
        return Err("poor");
    }
    st["magic"] = json!(n(st, "magic") - next.price);
    st["bag"]["tier"] = json!((tier + 1) as i64);
    Ok(
        json!({"ok": true, "cap": next.cap, "cost": next.price, "tier": tier + 1}),
    )
}

pub fn unlock_auto(st: &mut Value) -> Result<Value, &'static str> {
    if st["autoUnlocked"].as_bool().unwrap_or(false) {
        return Err("owned");
    }
    let price = data().k.auto_unlock_price;
    if n(st, "magic") < price {
        return Err("poor");
    }
    st["magic"] = json!(n(st, "magic") - price);
    st["autoUnlocked"] = json!(true);
    Ok(json!({"ok": true, "cost": price}))
}

pub fn refill_stamina(st: &mut Value, now: i64) -> Result<Value, &'static str> {
    recover_stamina(st, now);
    let need = data().k.stamina_max - n(&st["stamina"], "v");
    if need <= 0 {
        return Err("full");
    }
    let cost = data().k.stamina_refill_cost;
    if n(st, "magic") < cost {
        return Err("poor");
    }
    st["magic"] = json!(n(st, "magic") - cost);
    st["stamina"]["v"] = json!(data().k.stamina_max);
    st["stamina"]["at"] = json!(now);
    Ok(json!({"ok": true, "cost": cost}))
}

/* ================= 抽鱼 ================= */
