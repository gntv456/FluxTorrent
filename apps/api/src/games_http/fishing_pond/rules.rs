//! 甜梦钓场：所有会改存档的动作都在这里（服务端唯一写入点）。
//!
//! 抛竿抽鱼、结算发奖、商店扣费都由服务端掷骰/校验；客户端只负责表现与溜鱼手感。

use serde_json::{json, Value};

use super::data::data;
use super::state::*;

type Rng = dyn FnMut() -> f64;

/* ================= 装备 / 商城 ================= */

/* ================= 商城 ================= */
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

pub fn pool_of(st: &Value) -> Vec<&'static super::data::Fish> {
    let scene = s(st, "scene");
    let cond = s(st, "cond");
    let float_id = s(&st["equipped"], "float");
    let mut list: Vec<&super::data::Fish> = data()
        .fish
        .iter()
        .filter(|f| f.scene == scene && f.cond == cond)
        .filter(|f| match &f.need_float {
            Some(req) => req == float_id,
            None => true,
        })
        .collect();
    if list.is_empty() {
        list = data().fish.iter().filter(|f| f.scene == scene).collect();
    }
    list
}

use super::bait::rarity_weights;

pub fn roll_fish(st: &Value, rng: &mut Rng) -> &'static super::data::Fish {
    let stats = stats_of(st);
    let pool = pool_of(st);
    let d = data();
    for _ in 0..40 {
        let weights = rarity_weights(st, &pool, &stats);
        let total: f64 = weights.iter().sum();
        let mut pick = rng() * total;
        let mut chosen = "common".to_string();
        for (i, w) in weights.iter().enumerate() {
            if pick < *w {
                chosen = d.rarity_order[i].clone();
                break;
            }
            pick -= *w;
        }
        let list: Vec<&&super::data::Fish> =
            pool.iter().filter(|f| f.rarity == chosen).collect();
        if !list.is_empty() {
            let idx = ((rng() * list.len() as f64).floor() as usize)
                .min(list.len() - 1);
            return list[idx];
        }
    }
    pool.first().copied().unwrap_or(&d.fish[0])
}

/* ================= 抛竿 / 结算 ================= */

pub fn cast(
    st: &mut Value,
    now: i64,
    rng: &mut Rng,
) -> Result<Value, &'static str> {
    recover_stamina(st, now);
    let stats = stats_of(st);
    if n(&st["stamina"], "v") < data().k.cast_stamina_cost
        && n(st, "freeCasts") <= 0
    {
        return Err("noStamina");
    }
    super::bait::consume_for_cast(st)?;

    if n(st, "freeCasts") > 0 {
        st["freeCasts"] = json!(n(st, "freeCasts") - 1);
    } else {
        st["stamina"]["v"] =
            json!(n(&st["stamina"], "v") - data().k.cast_stamina_cost);
    }
    let base = 2400.0 + rng() * 2600.0;
    let cond_boost = if s(st, "cond") == "rain" { 0.15 } else { 0.0 };
    let bite_after =
        (base / (1.0 + stats.bite_speed + cond_boost)).round() as i64;
    let window_ms = stats.window.round() as i64;
    let fish = roll_fish(st, rng);
    let size = roll_size(fish, rng);
    // 饵效果只活在抽鱼那一瞬，不落库
    st["baitEff"] = json!({});
    st["stats"]["casts"] = json!(n(&st["stats"], "casts") + 1);
    st["stats"]["plays"] = json!(n(&st["stats"], "plays") + 1);
    Ok(json!({
        "fish": fish_json(fish),
        "size": size,
        "bite_after_ms": bite_after,
        "window_ms": window_ms,
        "stats": stats.to_json(),
        "limit": stats.limit,
        "pull": stats.pull
    }))
}

pub fn fish_json(f: &super::data::Fish) -> Value {
    json!({
        "id": f.id, "name": f.name, "scene": f.scene, "cond": f.cond,
        "rarity": f.rarity, "value": f.value, "river": f.river,
        "needFloat": f.need_float
    })
}

pub fn roll_size(f: &super::data::Fish, rng: &mut Rng) -> f64 {
    let span = (f.size[1] - f.size[0]).max(0.01);
    round2(f.size[0] + rng() * span)
}

/// 从落库的 cast_stats 还原结算需要的字段（结算只看 all / limit）。
pub fn stats_from_cast(v: &Value, all: f64) -> Stats {
    Stats {
        all,
        limit: v.get("limit").and_then(Value::as_f64).unwrap_or(100.0),
        pull: v.get("pull").and_then(Value::as_f64).unwrap_or(1.0),
        set_id: v.get("setId").and_then(Value::as_str).map(str::to_string),
        ..Default::default()
    }
}

/// 结算一尾：体型、魔力、图鉴、订单、统计、赛季。
pub fn settle_catch(
    st: &mut Value,
    fish: &super::data::Fish,
    size: f64,
    stats: &Stats,
    now: i64,
    rng: &mut Rng,
) -> Value {
    let span = (fish.size[1] - fish.size[0]).max(0.01);
    let size_factor = 0.8 + (size - fish.size[0]) / span * 0.8;
    let magic = ((fish.value as f64 * size_factor * (1.0 + stats.all)).round()
        as i64)
        .max(6);
    let weight = round2(size * (0.02 + rng() * 0.03));
    // `magic` 只是这一尾的**估值**（图鉴「单尾最高」/详情页用），不再入账：
    // 钓场是魔力的回收口（抛竿/装备/扩容只扣不返），渔获本身不发任何魔力。
    let entry = json!({
        "uid": format!("f{}{}", now, (rng() * 1e4) as i64),
        "id": fish.id, "name": fish.name, "rarity": fish.rarity,
        "size": size, "weight": weight, "magic": magic,
        "scene": s(st, "scene"), "cond": s(st, "cond"), "at": now
    });
    let dest = place_catch(st, entry.clone());
    let is_night = s(st, "cond") == "night";
    let is_rain = s(st, "cond") == "rain";
    let st_stats = &mut st["stats"];
    st_stats["catches"] = json!(n(st_stats, "catches") + 1);
    // 口径变更：`magicEarned` 现在是「累计渔获**估值**」（成就 magic20k 仍读它），
    // 不再对应任何入账 —— 钓场任何路径都不发魔力。
    st_stats["magicEarned"] = json!(n(st_stats, "magicEarned") + magic);
    let streak = n(st_stats, "curStreak") + 1;
    st_stats["curStreak"] = json!(streak);
    st_stats["bestStreak"] = json!(n(st_stats, "bestStreak").max(streak));
    if matches!(fish.rarity.as_str(), "rare" | "epic" | "legend") {
        st_stats["rareCaught"] = json!(n(st_stats, "rareCaught") + 1);
    }
    if fish.rarity == "epic" {
        st_stats["epicCaught"] = json!(n(st_stats, "epicCaught") + 1);
    }
    if fish.rarity == "legend" {
        st_stats["legendCaught"] = json!(n(st_stats, "legendCaught") + 1);
    }
    if is_night {
        st_stats["nightCatches"] = json!(n(st_stats, "nightCatches") + 1);
    }
    if is_rain {
        st_stats["rainCatches"] = json!(n(st_stats, "rainCatches") + 1);
    }
    if size >= 60.0 {
        st_stats["bigCaught"] = json!(n(st_stats, "bigCaught") + 1);
    }

    let prev = st["album"].get(&fish.id).cloned();
    let is_new = prev.is_none();
    let pc = prev.as_ref().map_or(0, |p| n(p, "count"));
    let pb = prev.as_ref().map_or(0, |p| n(p, "best"));
    let pbs = prev.as_ref().map(|p| f(p, "bestSize")).unwrap_or(0.0);
    st["album"][fish.id.as_str()] = json!({
        "count": pc + 1,
        "first": prev.as_ref().and_then(|p| p.get("first")).cloned().unwrap_or(json!(now)),
        "best": pb.max(magic),
        "bestSize": pbs.max(size),
        "scene": s(st, "scene")
    });
    let order_hits = bump_orders(st, &fish.id);
    json!({
        "entry": entry, "dest": dest, "magic": magic, "isNew": is_new,
        "size": size, "weight": weight,
        "albumCount": st["album"].as_object().map_or(0, |m| m.len() as i64),
        "orderHits": order_hits,
        "id": fish.id, "name": fish.name, "rarity": fish.rarity,
        "bagText": {"used": bag_used(st), "cap": bag_cap(st)}
    })
}

pub fn settle_miss(st: &mut Value, kind: &str) {
    st["stats"]["curStreak"] = json!(0);
    if kind == "break" {
        st["stats"]["breaks"] = json!(n(&st["stats"], "breaks") + 1);
    } else {
        st["stats"]["missed"] = json!(n(&st["stats"], "missed") + 1);
    }
}

/* ================= 订单 / 成就 / 鱼缸 ================= */

pub fn claim_order(st: &mut Value, idx: usize) -> Result<Value, &'static str> {
    let reward = {
        let Some(o) = st["orders"]["list"]
            .as_array_mut()
            .and_then(|a| a.get_mut(idx))
        else {
            return Err("missing");
        };
        if o["claimed"].as_bool().unwrap_or(false) {
            return Err("claimed");
        }
        if n(o, "got") < n(o, "need") {
            return Err("progress");
        }
        o["claimed"] = json!(true);
        n(o, "reward")
    };
    // 订单奖励不再入账魔力（只记估值与完成数）：钓场是回收口，不出水。
    st["stats"]["magicEarned"] = json!(n(&st["stats"], "magicEarned") + reward);
    st["stats"]["ordersDone"] = json!(n(&st["stats"], "ordersDone") + 1);
    Ok(json!({"ok": true, "reward": reward, "rewardKind": "score"}))
}

pub fn claim_order_bonus(st: &mut Value) -> Result<Value, &'static str> {
    let done = st["orders"]["list"]
        .as_array()
        .map(|a| {
            !a.is_empty()
                && a.iter().all(|o| o["claimed"].as_bool().unwrap_or(false))
        })
        .unwrap_or(false);
    if !done {
        return Err("progress");
    }
    if st["orders"]["bonusClaimed"].as_bool().unwrap_or(false) {
        return Err("claimed");
    }
    let reward = data().k.order_all_bonus;
    st["orders"]["bonusClaimed"] = json!(true);
    st["stats"]["magicEarned"] = json!(n(&st["stats"], "magicEarned") + reward);
    Ok(json!({"ok": true, "reward": reward, "rewardKind": "score"}))
}

pub fn claim_ach(
    st: &mut Value,
    id: &str,
    now: i64,
) -> Result<Value, &'static str> {
    let Some(a) = ach_claimable(st, id) else {
        return Err("locked");
    };
    st["ach"][id] = json!({"claimed": true, "at": now});
    st["stats"]["magicEarned"] =
        json!(n(&st["stats"], "magicEarned") + a.reward);
    Ok(json!({"ok": true, "reward": a.reward, "rewardKind": "score"}))
}

pub fn release_from_bag(
    st: &mut Value,
    uid: &str,
) -> Result<Value, &'static str> {
    let Some(arr) = st["bag"]["fish"].as_array_mut() else {
        return Err("missing");
    };
    let Some(pos) = arr.iter().position(|x| s(x, "uid") == uid) else {
        return Err("missing");
    };
    let fish = arr.remove(pos);
    Ok(json!({"ok": true, "fish": fish}))
}

pub fn take_from_aquarium(
    st: &mut Value,
    uid: &str,
) -> Result<Value, &'static str> {
    if bag_free(st) <= 0 {
        return Err("full");
    }
    let Some(arr) = st["aquarium"].as_array_mut() else {
        return Err("missing");
    };
    let Some(pos) = arr.iter().position(|x| s(x, "uid") == uid) else {
        return Err("missing");
    };
    let fish = arr.remove(pos);
    if let Some(bag) = st["bag"]["fish"].as_array_mut() {
        bag.push(fish.clone());
    }
    Ok(json!({"ok": true, "fish": fish}))
}

pub fn release_from_aquarium(
    st: &mut Value,
    uid: &str,
) -> Result<Value, &'static str> {
    let Some(arr) = st["aquarium"].as_array_mut() else {
        return Err("missing");
    };
    let Some(pos) = arr.iter().position(|x| s(x, "uid") == uid) else {
        return Err("missing");
    };
    let fish = arr.remove(pos);
    Ok(json!({"ok": true, "fish": fish}))
}

pub fn take_all_from_aquarium(st: &mut Value) -> Value {
    let mut moved = 0;
    loop {
        if bag_free(st) <= 0
            || st["aquarium"].as_array().map_or(true, |a| a.is_empty())
        {
            break;
        }
        let fish = st["aquarium"].as_array_mut().and_then(|a| {
            if a.is_empty() {
                None
            } else {
                Some(a.remove(0))
            }
        });
        if let Some(f) = fish {
            if let Some(bag) = st["bag"]["fish"].as_array_mut() {
                bag.push(f);
                moved += 1;
            }
        }
    }
    let left = st["aquarium"].as_array().map_or(0, |a| a.len()) as i64;
    json!({"ok": moved > 0, "moved": moved, "left": left})
}
