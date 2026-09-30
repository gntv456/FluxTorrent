//! 土地阶梯的关闸测试：钉住「升级只买周转、绝不买价值比」这条线。

use super::farm::crop_expected_value;
use super::farm::BASE_EV;
use super::farm::PLOTS as FARM_PLOTS;
use super::farm_land::*;

/// 等级必须单调买到更快的周转，且每一级都真的买到东西
#[test]
fn speed_is_strictly_better_until_the_cap() {
    let mut prev = speed_permille(1);
    assert_eq!(prev, SPEED_BASE_PERMILLE, "未升级就是原速");
    for level in 2..=MAX_LEVEL {
        let now = speed_permille(level);
        assert!(now < prev, "第 {level} 级没有比上一级快：{now} >= {prev}");
        assert!(
            now > SPEED_FLOOR_PERMILLE,
            "第 {level} 级已经踩到地板，说明上限给多了"
        );
        prev = now;
    }
}

/// 上限不是手写的数：它必须正好是「地板压下来之前」那一级 ——
/// 满级再往上一档，系数就撞死在地板上（等于那一注钱什么都没买到）
#[test]
fn the_cap_is_exactly_where_the_floor_binds() {
    assert!(speed_permille(MAX_LEVEL) > SPEED_FLOOR_PERMILLE);
    assert_eq!(speed_permille(MAX_LEVEL + 1), SPEED_FLOOR_PERMILLE);
    assert_eq!(
        validate_upgrade(MAX_LEVEL),
        Err(LandError::MaxLevel { level: MAX_LEVEL })
    );
    assert!(validate_upgrade(MAX_LEVEL - 1).is_ok());
    assert_eq!(MAX_LEVEL, 12, "上限变了要同步改地板口径的说明");
}

/// 成熟时长随等级单调下降，但**价值侧一个字都不动**：
/// 同一株作物在任何等级上的收获期望都还是作物表标定的 0.90。
/// 这条断言是整刀的护栏 —— 谁把等级接到产量或彩蛋倍率上，这里就红。
#[test]
fn upgrading_buys_turnaround_not_value() {
    for (seed_price, base_yield) in [(100, 75), (300, 225), (5000, 3750)] {
        let ev = crop_expected_value(seed_price, base_yield);
        assert!(
            (ev - BASE_EV).abs() < 1e-9,
            "标定被改动：{seed_price}/{base_yield} 的期望是 {ev}"
        );
        let mut prev = i64::MAX;
        for level in 1..=MAX_LEVEL {
            let mins = grow_minutes(4, level);
            assert!(mins < prev, "第 {level} 级的成熟时长没有变短");
            prev = mins;
        }
    }
}

/// 一小时的作物升到满级也不能被压成「种下即熟」
#[test]
fn grow_minutes_never_reaches_zero() {
    assert_eq!(grow_minutes(1, MAX_LEVEL), 30);
    assert!(grow_minutes(0, MAX_LEVEL) >= 1);
    assert_eq!(grow_minutes(4, 1), 240);
}

/// 阶梯在整个可买区间内严格变贵：否则「越买越便宜」会在某一级静默发生
#[test]
fn ladders_grow_strictly_over_the_whole_range() {
    let mut prev = 0;
    for purchased in 0..MAX_PLOTS_HARD_CAP {
        let p = land_price(DEFAULT_LAND_BASE, DEFAULT_LAND_RATIO, purchased);
        assert!(p > prev, "第 {} 块地不比上一块贵", purchased + 1);
        prev = p;
    }
    prev = 0;
    for level in 1..=MAX_LEVEL {
        let p = upgrade_price(DEFAULT_UP_BASE, DEFAULT_UP_RATIO, level);
        assert!(p > prev, "第 {level} 级升级费没有变贵");
        prev = p;
    }
    // 第一块买地钱与第一次升级钱就是各自的底数
    assert_eq!(land_price(DEFAULT_LAND_BASE, DEFAULT_LAND_RATIO, 0), 2000);
    assert_eq!(upgrade_price(DEFAULT_UP_BASE, DEFAULT_UP_RATIO, 1), 1000);
    // 阶梯走到硬上限那一档仍是有限正数（saturating，不会溢出成负价）
    let last = land_price(DEFAULT_LAND_BASE, DEFAULT_LAND_RATIO, 29);
    assert!(last > 0 && last < i64::MAX, "第 30 块地报价 {last}");
}

/// 地块只能按阶梯连续往外买：跳号、买已有、买满都要点名，不能静默成功
#[test]
fn purchases_are_contiguous_and_bounded() {
    // 免费 6 块，站长上限 12 块
    assert_eq!(next_purchasable_slot(6, 0, 12), Ok(7));
    assert_eq!(next_purchasable_slot(6, 5, 12), Ok(12));
    assert_eq!(
        next_purchasable_slot(6, 6, 12),
        Err(LandError::NoRoom { owned: 12, cap: 12 })
    );
    // 免费地不需要购买，重复购买要被点名而不是「成功」
    assert_eq!(
        validate_purchase(3, 6, 0, 12),
        Err(LandError::AlreadyOwned(3))
    );
    assert_eq!(
        validate_purchase(9, 6, 0, 12),
        Err(LandError::BadSlot { slot: 9, next: 7 })
    );
    assert_eq!(validate_purchase(7, 6, 0, 12), Ok(7));
    // 上限以下也没人能从第 7 块跳到第 13 块
    assert_eq!(
        validate_purchase(13, 6, 6, 12),
        Err(LandError::NoRoom { owned: 12, cap: 12 })
    );
}

/// 站长参数越界必须**指名道姓**地拒绝，而不是被 clamp 成一个能用的数
#[test]
fn config_out_of_range_is_named() {
    assert_eq!(
        validate_ladder(0, DEFAULT_LAND_RATIO, 12),
        Err(LandError::BadBase(0))
    );
    assert_eq!(
        validate_ladder(-5, DEFAULT_LAND_RATIO, 12),
        Err(LandError::BadBase(-5))
    );
    // 比率 1000‰ = 阶梯不涨；900‰ = 越买越便宜
    assert_eq!(validate_ladder(2000, 1000, 12), Err(LandError::BadRatio(1000)));
    assert_eq!(validate_ladder(2000, 900, 12), Err(LandError::BadRatio(900)));
    assert_eq!(validate_ladder(2000, RATIO_MIN, 12), Ok(()));
    assert_eq!(validate_ladder(2000, 2200, 0), Err(LandError::BadCap(0)));
    assert_eq!(
        validate_ladder(2000, 2200, i64::from(MAX_PLOTS_HARD_CAP) + 1),
        Err(LandError::BadCap(i64::from(MAX_PLOTS_HARD_CAP) + 1))
    );
    assert_eq!(validate_ladder(1, 1001, 1), Ok(()));
    // 地块永远不许白送（底数 0 也收 1 魔力）；这种配置在上面就被拒了
    assert_eq!(ladder_price(0, 2200, 0), 1);
}

/// `land_config` 是 site_settings 落到可用配置的**唯一**入口：
/// 越界必须在 i64 区间里点名，不能被 `as i32` 绕回合法区间
#[test]
fn config_landing_checks_the_i64_range() {
    let ok = land_config(12, 2000, 2200, 1000, 1600).unwrap();
    assert_eq!(
        ok,
        LandConfig {
            max_plots: 12,
            land_base: 2000,
            land_ratio: 2200,
            up_base: 1000,
            up_ratio: 1600,
        }
    );
    // 3e9 若先 as i32 会绕成负数；2^31+5 会绕成 5 —— 两者都要在这里被拒
    assert_eq!(
        land_config(3_000_000_000, 2000, 2200, 1000, 1600),
        Err(LandError::BadCap(3_000_000_000))
    );
    assert_eq!(
        land_config(2_147_483_653, 2000, 2200, 1000, 1600),
        Err(LandError::BadCap(2_147_483_653))
    );
    // 上限低于免费地块数：配置自己说不通
    assert_eq!(
        land_config(3, 2000, 2200, 1000, 1600),
        Err(LandError::CapBelowFree {
            cap: 3,
            free: FARM_PLOTS
        })
    );
    // 买地阶梯合法但升级阶梯配坏，同样整份拒绝（不能一半生效）
    assert_eq!(
        land_config(12, 2000, 2200, 0, 1600),
        Err(LandError::BadBase(0))
    );
    assert_eq!(
        land_config(12, 2000, 2200, 1000, 900),
        Err(LandError::BadRatio(900))
    );
    // 满打满算：上限就是硬上限，且价在这个区间内不溢出
    let maxed = land_config(
        i64::from(MAX_PLOTS_HARD_CAP),
        DEFAULT_LAND_BASE,
        DEFAULT_LAND_RATIO,
        DEFAULT_UP_BASE,
        DEFAULT_UP_RATIO,
    )
    .unwrap();
    assert_eq!(maxed.max_plots, MAX_PLOTS_HARD_CAP);
}

/// 报错文本要能读懂并且带上实数 —— 配置面直接把这句话念给站长看
#[test]
fn errors_name_the_offending_number() {
    let s = LandError::NoRoom { owned: 12, cap: 12 }.to_string();
    assert!(s.contains("12/12"), "{s}");
    let s = LandError::BadSlot { slot: 9, next: 7 }.to_string();
    assert!(s.contains("第 7 号") && s.contains("第 9 号"), "{s}");
    let s = LandError::BadRatio(1000).to_string();
    assert!(s.contains("1000"), "{s}");
    let s = LandError::MaxLevel { level: MAX_LEVEL }.to_string();
    assert!(s.contains(&MAX_LEVEL.to_string()), "{s}");
    let s = LandError::BadCap(0).to_string();
    assert!(s.contains("1 ~ 30"), "{s}");
}
