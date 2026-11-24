//! 甜梦钓场**经济口径**单测：钓场是魔力的回收口，任何路径都不返魔力。
//!
//! 2026-11-24 之前 `mirror_magic` 把正差额入账，起手白送 3200 + 每竿再赚
//! 21~610 魔力（每天约 240 竿体力）—— 一个无成本的增发口。下面每个测试
//! 锁一条「出水口已关」：渔获 / 订单 / 全清奖 / 成就 / 签到 / 起手赠金。

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
fn starter_state_is_playable_and_gives_no_magic() {
    let s = st::default_state(NOW);
    // 起手不送魔力：此前 3200 是白给的增发口（它会被镜像成真魔力）
    assert_eq!(n(&s, "magic"), 0);
    assert_eq!(n(&s["stamina"], "v"), 12);
    assert_eq!(n(&s, "freeCasts"), 3, "开荒靠免费竿，不靠赠金");
    assert_eq!(st::s(&s["equipped"], "rod"), "bamboo");
    assert_eq!(st::bag_cap(&s), 30);
}

#[test]
fn settle_fills_bag_album_and_stats_but_never_mints() {
    let mut s = funded(500);
    let fish = data().fish.iter().find(|f| f.scene == "bay").unwrap();
    let mut r = seq(&[0.9, 0.1]);
    let item =
        settle_catch(&mut s, fish, fish.size[1], &plain_stats(), NOW, &mut r);
    // 估值仍在（图鉴「单尾最高」/详情页读它）……
    assert!(item["magic"].as_i64().unwrap() >= 6);
    // ……但它不进余额：钓到鱼不是出水口
    assert_eq!(n(&s, "magic"), 500, "渔获不得让魔力变多");
    assert_eq!(s["bag"]["fish"].as_array().unwrap().len(), 1);
    assert!(s["album"].get(&fish.id).is_some());
    assert_eq!(n(&s["stats"], "catches"), 1);
    assert_eq!(n(&s["stats"], "curStreak"), 1);
    // magicEarned 改记「累计渔获估值」，成就 magic20k 仍读它
    assert_eq!(
        n(&s["stats"], "magicEarned"),
        item["magic"].as_i64().unwrap()
    );
}

/// 全鱼种 × 三个场景池连钓：余额一分不涨。
#[test]
fn no_path_mints_from_any_fish_in_any_scene() {
    let mut s = funded(1234);
    s["scenesOwned"] = json!(["bay", "gorge", "abyss", "reef"]);
    let mut r = seq(&[0.13, 0.71, 0.42, 0.9, 0.05]);
    for scene in ["bay", "gorge", "abyss", "reef"] {
        s["scene"] = json!(scene);
        for cond in ["day", "night", "rain"] {
            s["cond"] = json!(cond);
            for _ in 0..40 {
                let f = roll_fish(&s, &mut r);
                let sz = roll_size(f, &mut r);
                settle_catch(&mut s, f, sz, &plain_stats(), NOW, &mut r);
            }
        }
    }
    assert_eq!(n(&s, "magic"), 1234);
    assert!(n(&s["stats"], "catches") >= 480);
}

#[test]
fn orders_and_full_clear_bonus_never_mint() {
    let mut s = funded(777);
    // 把三单都刷成「已完成未领取」
    let list = s["orders"]["list"].as_array_mut().unwrap();
    for o in list.iter_mut() {
        let need = n(o, "need");
        o["got"] = json!(need);
    }
    let total: i64 = s["orders"]["list"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| n(o, "reward"))
        .sum();
    assert!(total > 0, "订单奖励额度应大于 0（它只记估值）");
    for i in 0..3 {
        let r = claim_order(&mut s, i).expect("claim ok");
        assert_eq!(r["rewardKind"], "score", "订单奖励是估值不是魔力");
    }
    let b = claim_order_bonus(&mut s).expect("bonus ok");
    assert_eq!(b["rewardKind"], "score");
    assert_eq!(n(&s, "magic"), 777, "订单 + 全清奖不得让魔力变多");
    assert_eq!(n(&s["stats"], "ordersDone"), 3);
}

#[test]
fn achievements_never_mint() {
    let mut s = funded(99);
    let mut claimed = 0;
    for a in &data().ach {
        // 直接把指标刷到达标，逐个领取
        if a.metric == "sceneCount"
            || a.metric == "bagTier"
            || a.metric == "gearOwned"
            || a.metric == "maxLv"
        {
            continue; // 结构类指标由别的测试覆盖，这里只验「领了不加钱」
        }
        s["stats"][a.metric.as_str()] = json!(a.target);
        if claim_ach(&mut s, &a.id, NOW).is_ok() {
            claimed += 1;
        }
    }
    assert!(claimed >= 10, "应至少领到 10 枚成就，实得 {claimed}");
    assert_eq!(n(&s, "magic"), 99, "成就不得让魔力变多");
}

#[test]
fn sign_in_pays_free_casts_never_magic() {
    let mut s = funded(0);
    let before_free = n(&s, "freeCasts");
    let mut got_magic_days = 0;
    // 走满两轮签到：覆盖魔力日 / 体力日 / 鱼饵日 / 传说饵箱全部档位
    for day in 0..14 {
        let (rw, _) = super::sign::sign_claim(&mut s, NOW + day * st::DAY_MS)
            .expect("sign ok");
        assert_eq!(
            n(&s, "magic"),
            0,
            "第 {day} 天签到不得发魔力（{:?}）",
            rw["type"]
        );
        if rw["type"] == "bait" {
            got_magic_days += 1;
        }
    }
    assert!(n(&s, "freeCasts") > before_free, "签到应折成免费竿");
    assert!(got_magic_days >= 4, "魔力日/饵箱应折成鱼饵类型");
    // 同日不可重复
    assert_eq!(
        super::sign::sign_claim(&mut s, NOW + 13 * st::DAY_MS).unwrap_err(),
        "claimed"
    );
}

/// 签到折算：⌈魔力 ÷ 10 × 3⌉ 竿（补满 12 点体力 40 魔力）。
#[test]
fn sign_cast_conversion_rounds_up() {
    let mut s = funded(0);
    let rewards = &data().k.sign_in_rewards;
    let first_magic = rewards
        .iter()
        .find(|r| r.kind == "magic")
        .map(|r| r.value)
        .unwrap();
    let want = (first_magic * 3 + 9) / 10;
    let f0 = n(&s, "freeCasts");
    // 推进 idx 到第一枚魔力日
    let idx = rewards.iter().position(|r| r.kind == "magic").unwrap() as i64;
    s["sign"]["idx"] = json!(idx);
    super::sign::sign_claim(&mut s, NOW).expect("sign ok");
    assert_eq!(n(&s, "freeCasts") - f0, want.max(1));
}
