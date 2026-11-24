//! 钓场「打窝饵 + 放生渔运」单测（2026-11-24 思齐对标批）。
//!
//! 经济口径：高档打窝饵用**魔力购买**（主回收口，每竿消耗 1 枚）；
//! 放生得**渔运**（第二资源，只可兑换解锁，永不可换魔力）。
//! 蚯蚓打窝饵免费（按日拾取），保证免费玩家的基线体验不变。

use serde_json::json;

use super::bait::*;
use super::rules::*;
use super::state as st;

const NOW: i64 = 1_795_100_000_000;

fn funded(magic: i64) -> serde_json::Value {
    let mut s = st::default_state(NOW);
    st::ensure_equipped(&mut s, NOW);
    st::orders_ensure(&mut s, NOW);
    s["magic"] = json!(magic);
    s
}

fn seq(vals: &'static [f64]) -> impl FnMut() -> f64 {
    let mut i = 0usize;
    move || {
        let v = vals[i % vals.len()];
        i += 1;
        v
    }
}

#[test]
fn worm_bait_is_always_free() {
    let mut s = funded(0);
    s["baits"]["worm"] = json!(0);
    // 免费档不进库存账：0 存量也能一直抛（体力/免费竿照常节流）
    let mut r = seq(&[0.3, 0.5]);
    cast(&mut s, NOW, &mut r).expect("cast ok");
    cast(&mut s, NOW + 1_000, &mut seq(&[0.4, 0.6])).expect("cast ok");
    cast(&mut s, NOW + 2_000, &mut seq(&[0.5, 0.7])).expect("cast ok");
    assert_eq!(super::state::n(&s, "magic"), 0, "蚯蚓竿永不花魔力");
}

#[test]
fn premium_bait_is_bought_with_magic_and_consumed_per_cast() {
    let mut s = funded(20_000);
    // 买 3 枚红虫（2000/枚）：花 6000 魔力
    let r = buy_bait(&mut s, "bloodworm", 3).expect("buy ok");
    assert_eq!(r["cost"], 6_000);
    assert_eq!(super::state::n(&s, "magic"), 14_000);
    assert_eq!(super::state::n(&s["baits"], "bloodworm"), 3);
    // 挂上红虫抛竿：库存 -1，且不产生任何魔力回流
    select_bait(&mut s, "bloodworm").expect("select ok");
    let mut r2 = seq(&[0.2, 0.4, 0.6]);
    cast(&mut s, NOW, &mut r2).expect("cast ok");
    assert_eq!(super::state::n(&s["baits"], "bloodworm"), 2);
    assert_eq!(super::state::n(&s, "magic"), 14_000, "抛竿不入场魔力");
}

#[test]
fn premium_bait_without_stock_is_rejected_not_free() {
    let mut s = funded(0);
    select_bait(&mut s, "goldfish").expect_err("无库存不可选中");
    // 直接把选中态写进去（绕过 UI 口）也要被 cast 拒
    s["baitId"] = json!("goldfish");
    let mut r = seq(&[0.5]);
    assert_eq!(cast(&mut s, NOW, &mut r).unwrap_err(), "noBait");
}

#[test]
fn bait_boosts_rare_weights_for_that_cast_only() {
    // 同一随机序列下：黄鱼饵（legend +0.45）抽出的稀有度 ≥ 蚯蚓饵
    let mut plain = funded(0);
    plain["scenesOwned"] = json!(["bay", "gorge", "abyss", "reef"]);
    plain["scene"] = json!("reef");
    plain["cond"] = json!("night");
    let mut boosted = plain.clone();
    boosted["baits"]["goldfish"] = json!(99);
    boosted["baitId"] = json!("goldfish");
    let mut r1 = seq(&[0.9, 0.1, 0.5]);
    let mut r2 = seq(&[0.9, 0.1, 0.5]);
    let f_plain = cast(&mut plain, NOW, &mut r1).unwrap();
    let f_boost = cast(&mut boosted, NOW, &mut r2).unwrap();
    fn rar(v: &serde_json::Value) -> &str {
        v["fish"]["rarity"].as_str().unwrap()
    }
    let order = ["common", "fine", "rare", "epic", "legend"];
    let ip = order.iter().position(|x| *x == rar(&f_plain)).unwrap();
    let ib = order.iter().position(|x| *x == rar(&f_boost)).unwrap();
    assert!(
        ib >= ip,
        "高档饵不该比免费饵更差：{} vs {}",
        rar(&f_boost),
        rar(&f_plain)
    );
    // 饵效果不落库
    assert_eq!(boosted["baitEff"], json!({}));
}

#[test]
fn releasing_fish_grants_luck_not_magic() {
    let mut s = funded(500);
    s["bag"]["fish"] = json!([
        { "uid": "f1", "id": "band", "rarity": "common", "magic": 16 },
        { "uid": "f2", "id": "aurora", "rarity": "epic", "magic": 330 },
        { "uid": "f3", "id": "coralDragon", "rarity": "legend", "magic": 920 },
    ]);
    let r1 = release_for_luck(&mut s, "bag", "f1").expect("release");
    assert_eq!(r1["luck"], 1, "普通鱼 1 渔运");
    let r2 = release_for_luck(&mut s, "bag", "f2").expect("release");
    assert_eq!(r2["luck"], 20, "史诗鱼 20 渔运");
    let r3 = release_for_luck(&mut s, "bag", "f3").expect("release");
    assert_eq!(r3["luck"], 100, "传说鱼 100 渔运");
    assert_eq!(super::state::n(&s, "luck"), 121);
    assert_eq!(super::state::n(&s, "magic"), 500, "放生不碰魔力");
    assert_eq!(super::state::n(&s["stats"], "released"), 3);
    // 鱼缸也能放
    s["aquarium"] = json!([{ "uid": "a1", "rarity": "rare" }]);
    let r4 = release_for_luck(&mut s, "aquarium", "a1").expect("release");
    assert_eq!(r4["luck"], 5);
    assert_eq!(super::state::n(&s, "luck"), 126);
    // 不存在的鱼 / 非法来源
    assert!(release_for_luck(&mut s, "bag", "nope").is_err());
    assert!(release_for_luck(&mut s, "pocket", "f1").is_err());
}

#[test]
fn luck_buys_scene_unlocks_at_60_percent() {
    let mut s = funded(0);
    s["luck"] = json!(400);
    // gorge 魔力价 600 → 渔运价 360
    let r = luck_buy_scene(&mut s, "gorge").expect("buy gorge");
    assert_eq!(r["cost"], 360);
    assert_eq!(super::state::n(&s, "luck"), 40);
    let owned = s["scenesOwned"].as_array().unwrap();
    assert!(owned.iter().any(|x| x.as_str() == Some("gorge")));
    // 重复购买被拒
    assert_eq!(luck_buy_scene(&mut s, "gorge").unwrap_err(), "owned");
    // 余额不足（abyss 渔运价 900 > 剩 40）
    assert_eq!(luck_buy_scene(&mut s, "abyss").unwrap_err(), "poor");
    // 顺序解锁：没钱但跳更远的 reef 也仍按 locked 拒
    s["luck"] = json!(5_000);
    assert_eq!(luck_buy_scene(&mut s, "reef").unwrap_err(), "locked");
}

#[test]
fn luck_price_floors_and_free_stays_free() {
    assert_eq!(luck_price(0), 0);
    assert_eq!(luck_price(600), 360);
    assert_eq!(luck_price(1500), 900);
    assert_eq!(luck_price(3000), 1800);
    assert_eq!(luck_price(1), 1, "1 魔力项至少 1 渔运");
}

#[test]
fn luck_exchanges_extend_to_items_bag_and_auto() {
    let mut s = funded(0);
    s["luck"] = json!(3_000);
    // 装备：月光传说竿 1200 魔力 → 720 渔运（60%）
    let r = luck_buy_item(&mut s, "rod", "moon").expect("buy rod");
    assert_eq!(r["cost"], 720);
    assert!(super::state::is_owned(&s, "rod", "moon"));
    assert_eq!(super::state::n(&s, "luck"), 3_000 - 720);
    // 重复购买被拒
    assert_eq!(luck_buy_item(&mut s, "rod", "moon").unwrap_err(), "owned");
    // 背包扩容：800 魔力 → 480 渔运
    let b = luck_expand_bag(&mut s).expect("expand");
    assert_eq!(b["cost"], 480);
    assert_eq!(super::state::bag_cap(&s), 50, "tier 0→1 = 30→50 格");
    // 自动垂钓：1500 魔力 → 900 渔运（此时余额 3000-720-480=1800）
    let a = luck_unlock_auto(&mut s).expect("auto");
    assert_eq!(a["cost"], 900);
    assert!(s["autoUnlocked"].as_bool().unwrap());
    assert_eq!(super::state::n(&s, "luck"), 3_000 - 720 - 480 - 900);
    assert_eq!(super::state::n(&s, "magic"), 0, "渔运交易全程不碰魔力");
    // 余额不足与重复买断
    assert_eq!(luck_unlock_auto(&mut s).unwrap_err(), "owned");
    assert_eq!(luck_expand_bag(&mut s).unwrap_err(), "poor");
}

#[test]
fn buying_bait_validates_inputs() {
    let mut s = funded(100_000);
    assert_eq!(buy_bait(&mut s, "nope", 1).unwrap_err(), "notfound");
    assert_eq!(buy_bait(&mut s, "worm", 1).unwrap_err(), "free");
    let mut poor = funded(500);
    assert_eq!(buy_bait(&mut poor, "disguise", 1).unwrap_err(), "poor");
    // 超库存上限
    let mut full = funded(10_000_000);
    full["baits"]["insect"] = json!(998);
    assert_eq!(buy_bait(&mut full, "insect", 5).unwrap_err(), "max");
}
