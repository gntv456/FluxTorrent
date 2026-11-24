//! 甜梦钓场规则层回归单测（从 rules.rs 拆出：撞 300 行门禁）。
//! 经济口径的锁定测试在 pond_sink_tests.rs。

use serde_json::{json, Value};

use super::data::data;
use super::rules::{self, *};
use super::state::{self as st, is_owned, n, Stats};

const NOW: i64 = 1_795_000_000_000; // 2026-11-21 前后，UTC ms

/// 带余额的存档：`magic` 是站点魔力的镜像，起手 0，测试里按需充值。
fn funded(magic: i64) -> Value {
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

fn plain_stats() -> Stats {
    Stats {
        all: 0.0,
        limit: 100.0,
        pull: 1.0,
        ..Default::default()
    }
}

#[test]
fn cast_spends_free_first_and_rolls_scene_fish() {
    let mut s = funded(0);
    let mut r = seq(&[0.25, 0.4, 0.6]);
    let out = cast(&mut s, NOW, &mut r).expect("cast ok");
    assert_eq!(n(&s, "freeCasts"), 2);
    assert_eq!(n(&s["stamina"], "v"), 12, "免费次数优先，体力不动");
    assert_eq!(out["fish"]["scene"], "bay");
    assert!(out["window_ms"].as_i64().unwrap() >= 400);
    assert!(out["bite_after_ms"].as_i64().unwrap() >= 900);
    // 体力/免费竿本身不花魔力：抛竿不是扣款点（扣款点是补体力）
    assert_eq!(n(&s, "magic"), 0);
}

#[test]
fn cast_without_resource_is_rejected() {
    let mut s = funded(0);
    s["freeCasts"] = json!(0);
    s["stamina"]["v"] = json!(0);
    let mut r = seq(&[0.5]);
    assert_eq!(cast(&mut s, NOW, &mut r).unwrap_err(), "noStamina");
}

#[test]
fn buying_charges_and_marks_owned_once() {
    let mut s = funded(3200);
    let r = buy_item(&mut s, "bait", "worm", NOW).expect("buy ok");
    assert_eq!(r["cost"], 120);
    assert_eq!(n(&s, "magic"), 3080);
    assert!(is_owned(&s, "bait", "worm"));
    assert_eq!(buy_item(&mut s, "bait", "worm", NOW).unwrap_err(), "owned");
}

#[test]
fn buying_unknown_or_unaffordable_is_rejected() {
    let mut s = funded(3200);
    assert_eq!(
        buy_item(&mut s, "bait", "nope", NOW).unwrap_err(),
        "notfound"
    );
    s["magic"] = json!(10);
    assert_eq!(buy_item(&mut s, "rod", "moon", NOW).unwrap_err(), "poor");
}

#[test]
fn miss_resets_streak_and_counts() {
    let mut s = funded(0);
    s["stats"]["curStreak"] = json!(3);
    settle_miss(&mut s, "miss");
    assert_eq!(n(&s["stats"], "curStreak"), 0);
    assert_eq!(n(&s["stats"], "missed"), 1);
    settle_miss(&mut s, "break");
    assert_eq!(n(&s["stats"], "breaks"), 1);
}

#[test]
fn bag_and_auto_prices_match_client() {
    let mut s = funded(3200);
    expand_bag(&mut s).expect("expand ok");
    assert_eq!(n(&s, "magic"), 2400);
    assert_eq!(st::bag_cap(&s), 50);
    s["magic"] = json!(1600);
    unlock_auto(&mut s).expect("unlock ok");
    assert_eq!(n(&s, "magic"), 100);
    assert!(s["autoUnlocked"].as_bool().unwrap());
}

#[test]
fn refill_stamina_is_the_only_cast_cost() {
    let mut s = funded(40);
    s["stamina"]["v"] = json!(0);
    s["freeCasts"] = json!(0);
    // 体力为 0 且无免费竿 → 抛不了
    let mut r = seq(&[0.5]);
    assert_eq!(cast(&mut s, NOW, &mut r).unwrap_err(), "noStamina");
    // 40 魔力补满 → 扣款点在这里
    refill_stamina(&mut s, NOW).expect("refill ok");
    assert_eq!(n(&s, "magic"), 0);
    assert_eq!(n(&s["stamina"], "v"), data().k.stamina_max);
}

#[test]
fn upgrades_follow_rate_curve() {
    assert_eq!(upgrade_cost(300, 1), 135);
    assert_eq!(upgrade_cost(300, 2), 270);
    assert_eq!(upgrade_cost(0, 1), 54, "起步价 120 × 0.45");
}

#[test]
fn roll_size_stays_within_species_range() {
    let fish = &data().fish[0];
    for k in 0..20 {
        let mut i = 0u64;
        let mut r = move || {
            i += 1;
            (k as f64 + i as f64 * 0.1) / 20.0
        };
        let z = roll_size(fish, &mut r);
        assert!(z >= fish.size[0] && z <= fish.size[1]);
    }
}

#[test]
fn pool_depends_on_scene_and_unlock() {
    let mut s = funded(0);
    let bay = pool_of(&s);
    assert!(!bay.is_empty());
    assert!(bay.iter().all(|f| f.scene == "bay"));
    s["scenesOwned"] = json!(["bay", "gorge"]);
    s["scene"] = json!("gorge");
    let gorge = pool_of(&s);
    assert!(gorge.iter().all(|f| f.scene == "gorge"));
    assert!(!gorge.is_empty());
}

/// 老存档残留的 `exchange` 台账与白送余额：`hydrate` 只补缺不覆盖，
/// 所以清账必须在读档时做 —— 这条测试锁「读档后余额恒等于真实余额」
/// 这件事由 HTTP 层 `spark_of` 负责，规则层不持有任何「应发额度」。
#[test]
fn hydrate_does_not_invent_balance() {
    let legacy = json!({
        "v": 1, "magic": 3200,
        "exchange": { "day": "", "out": 0, "in": 0, "outTotal": 0 }
    });
    let s = st::hydrate(legacy, NOW);
    // hydrate 不改玩家已有值（3200 原样保留）——它不是出水口，
    // 因为 HTTP 层每次读档都会用真实余额覆盖 magic。
    assert_eq!(n(&s, "magic"), 3200);
    // 新存档不再携带从未被读写的 exchange 死字段
    assert!(st::default_state(NOW).get("exchange").is_none());
}
