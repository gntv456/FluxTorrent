//! 农场彩蛋池单测：锁的是「0.90 收获 + 彩蛋 < 1」这条**农场专属**的余量口径，
//! 以及作物标定（`validate_crop`）—— 池子结构项本身由 pool_tests 那边锁。

use super::*;

fn magic(label: &str, weight: u32, mult_permille: i64) -> PoolEntry {
    PoolEntry {
        label: label.to_string(),
        weight,
        kind: EntryKind::Magic { mult_permille },
    }
}

fn item(
    label: &str,
    weight: u32,
    key: &str,
    qty: i32,
    anchor: i64,
) -> PoolEntry {
    PoolEntry {
        label: label.to_string(),
        weight,
        kind: EntryKind::Item {
            item_key: key.to_string(),
            qty,
            anchor,
        },
    }
}

// ---- 农场收获彩蛋池：0.90 之外的余量只有 0.10 ----

/// 与 0252 播种同形的夹具：unit = 最便宜作物的种子价（这里取 80）
fn seeded_farm() -> Vec<PoolEntry> {
    vec![
        magic("本轮无额外奖励", 895, 0),
        magic("种子价 80% 彩头", 100, 800),
        item("抽卡券", 5, "pass3", 1, 160),
    ]
}

#[test]
fn farm_egg_seeded_shape_fits_the_0_10_margin() {
    let p = seeded_farm();
    validate_farm(&p, 80).expect("播种彩蛋池必须合法");
    // 彩头 0.10×0.8 = 0.08，物品 0.005×160/80 = 0.01 → 池子 0.09，总回收 0.99
    assert!(
        (farm_total_ev(&p, 80) - 0.99).abs() < 1e-9,
        "总回收漂移: {}",
        farm_total_ev(&p, 80)
    );
}

#[test]
fn farm_egg_pays_off_the_crop_you_harvest() {
    // 彩头按**这一株**的种子价折算：贵作物给得多，但比值与收获同 scale
    assert_eq!(egg_pay(80, 200), 16);
    assert_eq!(egg_pay(5_000, 200), 1_000);
    assert_eq!(egg_pay(80, 0), 0);
}

#[test]
fn farm_egg_rejects_pool_that_pushes_total_over_one() {
    // 单档 150% 种子价必中：池子 EV 才 1.5，但总回收 0.90+1.5 早穿 1 ——
    // 用 validate_pool 判会放行（它只看池子），农场这条闸必须自己拦
    let p = vec![magic("离谱彩头", 1000, 1_500)];
    assert!(validate_pool(&p, 80).is_err());
    let bad = vec![magic("刚过线", 1000, 112)];
    assert!(validate_pool(&bad, 80).is_ok(), "池子侧本身合法");
    assert!(validate_farm(&bad, 80).is_err(), "总回收 >= 1 必须拒");
    assert!(matches!(
        validate_farm(&bad, 80),
        Err(PoolError::ExpectedValueNotBelowOne(_))
    ));
}

#[test]
fn farm_egg_item_tier_shares_the_shape_gate() {
    // 物品位按 anchor 计入 EV，同一条 0.10 余量；anchor<=0 是空头承诺
    let ok = vec![
        magic("本轮无额外奖励", 950, 0),
        item("抽卡券", 50, "pass3", 1, 2),
    ];
    validate_farm(&ok, 80).expect("0.05 × 2/80 = 池子 0.00125");
    let broke = vec![
        magic("本轮无额外奖励", 500, 0),
        item("抽卡券", 500, "pass3", 1, 100),
    ];
    assert!(
        validate_farm(&broke, 80).is_err(),
        "0.5×100/80 = 0.625 穿线"
    );
    assert!(matches!(
        validate_farm(&[item("空目录", 1000, "", 1, 0)], 80),
        Err(PoolError::UnknownItem(_))
    ));
}

#[test]
fn farm_egg_item_tier_is_gated_by_the_cheapest_crop() {
    // 魔力位随种子价等比缩放，EV 与定标单位无关；**物品位是定值**，
    // 作物越便宜它占比越大 —— 所以单位必须取 min(seed_price)
    let p = vec![
        magic("本轮无额外奖励", 950, 0),
        item("抽卡券", 50, "pass3", 1, 100),
    ];
    assert!(validate_farm(&p, 5_000).is_ok(), "0.05×100/5000 = 0.001");
    assert!(validate_farm(&p, 80).is_ok(), "0.05×100/80 = 0.0625");
    // 新种一株 5 魔力的作物：同一档配置变成 0.05×100/5 = 1.0，必须当场拦住
    assert!(validate_farm(&p, 5).is_err());
    // 0 单位（作物表为空）不是「无风险」，是「无法定标」：直接拒
    assert!(matches!(validate_farm(&p, 0), Err(PoolError::BadTicket(0))));
}

// ---- 作物表标定：BASE_EV 不是假设，而是 validate_crop 钉住的天花板 ----

#[test]
fn crop_calibration_is_the_base_ev_ceiling() {
    // 播种的 100/75 正好压在标定上（产量 = 种子价 × 0.75）
    validate_crop(100, 75, 4).expect("播种档位必须合法");
    assert!(crop_expected_value(100, 75) <= super::farm::BASE_EV + 1e-12);
    // 多一颗产量就不是「更慷慨的作物」，而是农场开始增发
    assert!(matches!(
        validate_crop(100, 76, 4),
        Err(CropError::OverCalibration { .. })
    ));
    // 白送的种子是最坏情况，不是没情况
    assert!(matches!(
        validate_crop(0, 1, 4),
        Err(CropError::BadPrice(0))
    ));
    assert!(matches!(
        validate_crop(100, 0, 4),
        Err(CropError::BadYield(0))
    ));
    assert!(matches!(
        validate_crop(100, 75, 0),
        Err(CropError::BadGrow(0))
    ));
    assert!(validate_crop(100, 75, super::farm::GROW_HOURS_MAX).is_ok());
    // 列宽是 INT4：越界必须点名，不能让 `as i32` 绕成负数再撞 CHECK
    assert!(matches!(
        validate_crop(i64::from(i32::MAX) + 1, 10, 4),
        Err(CropError::BadPrice(_))
    ));
    assert!(matches!(
        validate_crop(100, i64::from(i32::MAX) + 1, 4),
        Err(CropError::BadYield(_))
    ));
}

#[test]
fn cheaper_crop_squeezes_the_egg_pool_through_the_unit() {
    // 同一张彩蛋池、同一件物品：定标单位（现役最便宜种子价）掉到 100 就穿线，
    // 停在 2000 还在闸内 —— 作物改动改的是**池子的余量**，所以保存必须回查
    let entries = vec![
        magic("本轮无额外奖励", 990, 0),
        item("免考核卡", 10, "pass3", 1, 12_000),
    ];
    assert!(
        validate_farm(&entries, 2_000).is_ok(),
        "0.01×12000/2000=0.06"
    );
    assert!(validate_farm(&entries, 100).is_err(), "0.01×12000/100=1.2");
}

#[test]
fn egg_exactly_at_the_margin_is_rejected() {
    // 余量 0.10 被**正好**用满 = 总回收恰为 1：中性池必须拒。
    // 这一条过去会被浮点放行（0.90 + 0.10 算成 0.9999999999999999），
    // 判据换成整数比较（`ev_strictly_below`）才真拦住 —— 别改回 `!(ev < 1.0)`。
    assert!(validate_farm(&[magic("刚好用满余量", 1000, 100)], 100).is_err());
    // 差一口气才是合法的回收口
    assert!(validate_farm(&[magic("留一口气", 1000, 99)], 100).is_ok());
    // 池子侧同一形状：EV 恰为 1 的中性池不许靠取整溜过去
    assert!(validate_pool(&[magic("中性池", 1000, 1000)], 100).is_err());
}
