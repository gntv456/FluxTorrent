//! 打窝饵（每竿消耗 1 枚）与放生渔运：纯规则层。

//! 经济口径（2026-11-24 思齐对标）：高档饵用**魔力购买**（主回收口），
//! 放生得**渔运**（第二资源，只可兑换解锁，永不可换魔力）。

use serde_json::{json, Value};

use super::data::data;
use super::state::Stats;
use super::state::{f, is_owned, n, s};

/// 买打窝饵：数量进库存、花站点魔力（主回收口）。免费档不可购买。
pub fn buy_bait(
    st: &mut Value,
    bait_id: &str,
    qty: i64,
) -> Result<Value, &'static str> {
    let Some(b) = data().bait(bait_id) else {
        return Err("notfound");
    };
    if b.recover || b.price == 0 {
        return Err("free");
    }
    let qty = qty.clamp(1, 99);
    let cost = b.price * qty;
    if n(st, "magic") < cost {
        return Err("poor");
    }
    let have = n(&st["baits"], &b.id);
    if have + qty > data().k.bait_max_stack {
        return Err("max");
    }
    st["magic"] = json!(n(st, "magic") - cost);
    st["baits"][&b.id] = json!(have + qty);
    Ok(json!({"ok": true, "cost": cost, "qty": qty, "id": b.id}))
}

/// 选竿上挂哪种打窝饵（免费蚯蚓随时可选；高档须有库存）。
pub fn select_bait(
    st: &mut Value,
    bait_id: &str,
) -> Result<Value, &'static str> {
    let Some(b) = data().bait(bait_id) else {
        return Err("notfound");
    };
    if !b.recover && b.price > 0 && n(&st["baits"], &b.id) < 1 {
        return Err("empty");
    }
    st["baitId"] = json!(b.id);
    Ok(json!({"ok": true, "id": b.id}))
}

/// 放生一尾（背包或鱼缸）得渔运：只计点数，不碰魔力。
/// 渔运只能用于兑换解锁（鱼竿/场景/扩容），永不可换回魔力。
pub fn release_for_luck(
    st: &mut Value,
    from: &str,
    uid: &str,
) -> Result<Value, &'static str> {
    let arr = match from {
        "bag" => &mut st["bag"]["fish"],
        "aquarium" => &mut st["aquarium"],
        _ => return Err("from"),
    };
    let Some(list) = arr.as_array_mut() else {
        return Err("missing");
    };
    let Some(pos) = list.iter().position(|x| s(x, "uid") == uid) else {
        return Err("missing");
    };
    let fish = list.remove(pos);
    let luck = data().release_luck(s(&fish, "rarity"));
    st["luck"] = json!(n(st, "luck") + luck);
    st["stats"]["released"] = json!(n(&st["stats"], "released") + 1);
    Ok(json!({"ok": true, "luck": luck, "fish": fish}))
}

/// 渔运兑换解锁：目前支持场景解锁（装备/扩容的渔运价后续按需扩展，
/// 口径=原魔力价的 60%，向下取整、至少 1）。
pub fn luck_buy_scene(
    st: &mut Value,
    scene_id: &str,
) -> Result<Value, &'static str> {
    let Some(sc) = data().scenes.iter().find(|s| s.id == scene_id) else {
        return Err("notfound");
    };
    if st["scenesOwned"]
        .as_array()
        .map_or(false, |a| a.iter().any(|x| x.as_str() == Some(scene_id)))
    {
        return Err("owned");
    }
    // 顺序解锁：前一场景未开不许跳
    if let Some(idx) = data().scenes.iter().position(|s| s.id == scene_id) {
        if idx > 0 {
            let prev = &data().scenes[idx - 1].id;
            let has = st["scenesOwned"]
                .as_array()
                .map_or(false, |a| a.iter().any(|x| x.as_str() == Some(prev)));
            if !has {
                return Err("locked");
            }
        }
    }
    let price = luck_price(sc.price);
    pay_luck(st, price)?;
    if let Some(arr) = st["scenesOwned"].as_array_mut() {
        arr.push(json!(scene_id));
    }
    Ok(json!({"ok": true, "cost": price, "scene": sc.name}))
}

/// 渔运买装备（按目录分类+物品键）：只解锁持有，不自动装备 ——
/// 装备仍走装备栏，保持「买」与「穿」两个动作的语义。
pub fn luck_buy_item(
    st: &mut Value,
    cat: &str,
    item_id: &str,
) -> Result<Value, &'static str> {
    let Some(list) = data().items.get(cat) else {
        return Err("notfound");
    };
    let Some(it) = list.iter().find(|i| i.id == item_id) else {
        return Err("notfound");
    };
    if is_owned(st, cat, item_id) {
        return Err("owned");
    }
    let price = luck_price(it.price);
    pay_luck(st, price)?;
    let entry = json!({ "id": it.id, "lv": 1, "at": 0 });
    match st["owned"][cat].as_array_mut() {
        Some(a) => a.push(entry),
        None => st["owned"][cat] = json!([entry]),
    }
    Ok(json!({"ok": true, "cost": price, "item": it.name, "cat": cat}))
}

/// 渔运扩背包：档位顺序与 `rules::expand_bag` 同一把尺（下一档价格 × 60%）。
pub fn luck_expand_bag(st: &mut Value) -> Result<Value, &'static str> {
    let tier = n(&st["bag"], "tier") as usize;
    let Some(next) = data().k.bag_tiers.get(tier + 1) else {
        return Err("max");
    };
    let price = luck_price(next.price);
    pay_luck(st, price)?;
    st["bag"]["tier"] = json!((tier + 1) as i64);
    Ok(json!({
        "ok": true, "cost": price, "cap": next.cap, "tier": tier + 1
    }))
}

/// 渔运买断自动垂钓（与 `rules::unlock_auto` 同口径）。
pub fn luck_unlock_auto(st: &mut Value) -> Result<Value, &'static str> {
    if st["autoUnlocked"].as_bool().unwrap_or(false) {
        return Err("owned");
    }
    let price = luck_price(data().k.auto_unlock_price);
    pay_luck(st, price)?;
    st["autoUnlocked"] = json!(true);
    Ok(json!({"ok": true, "cost": price}))
}

/// 扣渔运（余额不足统一报 poor）。
fn pay_luck(st: &mut Value, price: i64) -> Result<(), &'static str> {
    if price > 0 && n(st, "luck") < price {
        return Err("poor");
    }
    if price > 0 {
        st["luck"] = json!(n(st, "luck") - price);
    }
    Ok(())
}

/// 渔运价：原魔力价 × 60%，向下取整（免费项仍免费）。
pub fn luck_price(magic_price: i64) -> i64 {
    if magic_price <= 0 {
        0
    } else {
        ((magic_price * 3) / 5).max(1)
    }
}

/// 抛竿时的饵消耗：扣 1 枚高档饵（无库存拒竿并回落蚯蚓），
/// 再把本竿加成挂进 `baitEff`（`rarity_weights` 读它）。
/// 抽完鱼由 `cast` 清空 —— 饵效果不落库。
pub fn consume_for_cast(st: &mut Value) -> Result<(), &'static str> {
    let bait_id = s(st, "baitId").to_string();
    let bait = data().bait(&bait_id).filter(|b| !b.recover && b.price > 0);
    if let Some(b) = bait {
        let have = n(&st["baits"], &b.id);
        if have < 1 {
            st["baitId"] = json!("worm");
            return Err("noBait");
        }
        st["baits"][&b.id] = json!(have - 1);
        if have - 1 <= 0 {
            st["baitId"] = json!("worm");
        }
    }
    st["baitEff"] = bait
        .map(|b| {
            let mut m = serde_json::Map::new();
            for (k, v) in &b.eff {
                m.insert(k.clone(), json!(v));
            }
            Value::Object(m)
        })
        .unwrap_or_else(|| json!({}));
    Ok(())
}

/// 打窝饵对稀有度权重的三元加成 (rare, epic, legend)；epic 取 max(epic, rare)。
pub fn boosts(eff: &Value) -> (f64, f64, f64) {
    let g = |k: &str| f(eff, k).max(0.0);
    let rare = g("rare");
    (rare, g("epic").max(rare), g("legend"))
}

/// 稀有度权重（含打窝饵加成）：从 bait.rs 提供权重视角，
/// rules::roll_fish 消费。`stats` 由调用方算好传入。
pub fn rarity_weights(
    st: &Value,
    pool: &[&super::data::Fish],
    stats: &Stats,
) -> Vec<f64> {
    let night = s(st, "cond") == "night";
    let rare_boost = stats.rare * (if night { 1.0 + stats.night } else { 1.0 })
        + if night { stats.night * 0.4 } else { 0.0 };
    let legend_boost = stats.legend + rare_boost * 0.25;
    let boost = super::bait::boosts(&st["baitEff"]);
    let d = data();
    d.rarity_order
        .iter()
        .map(|rk| {
            if !pool.iter().any(|f| &f.rarity == rk) {
                return 0.0;
            }
            let base = d.rarity.get(rk).map(|r| r.weight).unwrap_or(0.0);
            match rk.as_str() {
                "rare" => base * (1.0 + rare_boost + boost.0),
                "epic" => base * (1.0 + rare_boost * 0.9 + boost.1),
                "legend" => base * (1.0 + legend_boost * 2.2 + boost.2),
                "common" => base * (1.0 + stats.common),
                _ => base,
            }
        })
        .collect()
}
