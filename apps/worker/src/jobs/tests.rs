//! 做种计费乘数单测（口径与迁移 0133 对账）。
//! 从 jobs.rs 按域拆出。

use super::billing::billing_multipliers;

/// 做种收益公式口径锁定：与迁移 0133 的 DB 函数（`seeding_torrent_bonus` / `seeding_hourly`）
/// 同构的 Rust 镜像。**公式的唯一权威在 DB**（worker 结算与 API 预估都调它），
/// 本镜像用于快速单测；两者一致性由 `_v_seeding.py` 用 180 组参数逐组对账（1e-9 容差）守住。
/// 常量必须与迁移 0133 的 site_settings 缺省值一致（vol_base 50 / rarity 0.6,0.35 /
/// scale 0.4 / cap 150 / curve_k 0.12）。
const VOL_BASE: f64 = 50.0;
const RARITY_K: f64 = 0.6;
const RARITY_EXP: f64 = 0.35;
const SCALE: f64 = 0.4;
const CAP: f64 = 150.0;
const CURVE_K: f64 = 0.12;
const GIB: f64 = 1073741824.0;

/// 种子维度档位分（第一命中优先）
fn rule_seed(size: i64, seeders: i64, age_days: f64, completed: i64) -> f64 {
    if seeders <= 1 && completed >= 3 {
        2.0
    } else if age_days > 365.0 {
        1.5
    } else if age_days > 180.0 {
        1.0
    } else if size as f64 >= 100.0 * GIB {
        0.75
    } else if size as f64 >= 25.0 * GIB {
        0.5
    } else {
        0.25
    }
}

/// 个人做种时长档位分（奖励长期保种，替代旧的 1/(1+h/2160) 衰减）
fn dur_bonus(personal_hours: f64) -> f64 {
    if personal_hours >= 8760.0 {
        2.0
    } else if personal_hours >= 4320.0 {
        1.0
    } else if personal_hours >= 2160.0 {
        0.75
    } else if personal_hours >= 720.0 {
        0.5
    } else {
        0.0
    }
}

fn torrent_bonus(
    size: i64,
    seeders: i64,
    age_days: f64,
    completed: i64,
    personal_hours: f64,
) -> f64 {
    // 与 DB 一致地用自然对数（比值等价，避免 log10/ln 混用造成对账口径分歧）
    let vol = ((1.0 + size as f64 / GIB).ln() / (1.0 + VOL_BASE).ln()).min(1.0);
    let rar = 1.0 + RARITY_K * (seeders.max(1) as f64).powf(-RARITY_EXP);
    (rule_seed(size, seeders, age_days, completed) + dur_bonus(personal_hours))
        * vol
        * rar
        * SCALE
}

/// (size, seeders, age_days, completed, personal_hours) → 每小时魔力（不含 donor 倍数）
fn user_hourly(base: f64, torrents: &[(i64, i64, f64, i64, f64)]) -> f64 {
    let sum: f64 = torrents
        .iter()
        .map(|(z, s, a, c, h)| torrent_bonus(*z, *s, *a, *c, *h))
        .sum();
    base + (2.0 / std::f64::consts::PI * CAP * (sum * CURVE_K).atan()).floor()
}

#[test]
fn seeding_formula_volume_monotonic_and_saturated() {
    // 体积单调且在对数基准（50GB）处饱和
    let g1 = torrent_bonus(1 * GIB as i64, 5, 60.0, 10, 100.0);
    let g10 = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 100.0);
    let g100 = torrent_bonus(100 * GIB as i64, 5, 60.0, 10, 100.0);
    let t1 = torrent_bonus(1024 * GIB as i64, 5, 60.0, 10, 100.0);
    assert!(g1 < g10 && g10 < g100, "{g1} {g10} {g100}");
    assert!((g100 - t1).abs() < 1e-12, "体积应饱和: {g100} vs {t1}");
}

#[test]
fn seeding_small_torrent_pile_is_not_profitable() {
    // 本次改造的核心：堆 1000 颗 1MB 小种只能拿底薪（旧公式可拿到 ~205/h）
    let pile: Vec<(i64, i64, f64, i64, f64)> =
        vec![(1048576, 1, 3.0, 0, 1.0); 1000];
    let pile_hourly = user_hourly(10.0, &pile);
    let honest: Vec<(i64, i64, f64, i64, f64)> =
        vec![(100 * GIB as i64, 1, 400.0, 5, 8760.0); 6];
    let honest_hourly = user_hourly(10.0, &honest);
    assert!(pile_hourly <= 12.0, "堆小种不应超出底薪: {pile_hourly}");
    assert!(honest_hourly >= 80.0, "老实保种应拿得多: {honest_hourly}");
    assert!(
        honest_hourly >= pile_hourly * 8.0,
        "{honest_hourly} vs {pile_hourly}"
    );
}

#[test]
fn seeding_personal_time_rewards_loyalty() {
    // 与旧公式相反的故意改动：挂得越久收益越高（对齐 Gazelle/U3D 的长期保种激励）
    let short = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 240.0);
    let mid = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 2400.0);
    let year = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 8760.0);
    assert!(short < mid && mid < year, "{short} {mid} {year}");
}

#[test]
fn seeding_rarity_bonus_not_penalty() {
    // 档位隔离（全是"日常种"）：独苗最高，人多趋近 1 而不是趋近 0
    let lone = torrent_bonus(10 * GIB as i64, 1, 60.0, 0, 100.0);
    let ten = torrent_bonus(10 * GIB as i64, 10, 60.0, 0, 100.0);
    let hundred = torrent_bonus(10 * GIB as i64, 100, 60.0, 0, 100.0);
    assert!(lone > ten && ten > hundred, "{lone} {ten} {hundred}");
    // 独苗/10 人 = rarity(1)/rarity(10) = 1.6 / 1.2679
    assert!(
        (lone / ten - 1.6 / (1.0 + 0.6 * 10.0_f64.powf(-0.35))).abs() < 1e-9
    );
    // 热门不再被重罚：100 人时仍保留 5 人时的 80% 以上（旧公式 ≈35%）
    let five = torrent_bonus(10 * GIB as i64, 5, 60.0, 0, 100.0);
    assert!(hundred / five > 0.80, "{}", hundred / five);
}

#[test]
fn seeding_formula_arctan_cap() {
    // 1000 颗濒危种也只在软封顶内（渐近 base+150，且 atan 开区间取不到）
    let many: Vec<(i64, i64, f64, i64, f64)> =
        vec![(100 * GIB as i64, 1, 400.0, 5, 8760.0); 1000];
    let total = user_hourly(10.0, &many);
    assert!(total < 10.0 + CAP, "total={total}");
    assert!(total > 10.0 + 100.0, "total={total}（应明显超过半程）");
}

#[test]
fn seeding_formula_dying_beats_daily() {
    // 濒危保种（档位 2.0）时薪远高于日常种（0.25）——激励方向正确。
    // 同为 10GB 种子以隔离体积因子：比值应为 2.0/0.25 = 8
    let dying = torrent_bonus(10 * GIB as i64, 1, 400.0, 5, 100.0);
    let daily = torrent_bonus(10 * GIB as i64, 1, 10.0, 1, 100.0);
    assert!(dying > daily * 7.0, "dying={dying} daily={daily}");
}

#[test]
fn free_zeroes_download() {
    assert_eq!(billing_multipliers(Some("free"), None), (1.0, 0.0));
}

#[test]
fn stronger_promotion_wins() {
    assert_eq!(billing_multipliers(Some("free"), Some("x2")), (2.0, 1.0));
    assert_eq!(
        billing_multipliers(Some("x2free"), Some("free")),
        (2.0, 0.0)
    );
}

#[test]
fn none_is_normal() {
    assert_eq!(billing_multipliers(None, None), (1.0, 1.0));
}
