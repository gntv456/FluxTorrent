//! `gacha_math` 的镜像用例 —— 与 `apps/web/tests/gacha-math.test.ts` 逐条对齐。
//! 共用向量 `gacha_math_vectors.json`（样张 oracle 生成；生成时已对样张逐位比对）。
//! f64 字段容差 1e-9；simulate 的整数计数逐位相等（mulberry32 纯整数流）。
//! 改任一侧实现或向量，都必须同步另一侧并重生成。

use super::{
    composite, economics, is_gold, simulate, PityConfig, PoolConfig, RateRow,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VectorFile {
    #[serde(default)]
    #[allow(dead_code)]
    meta: serde_json::Value,
    cases: Vec<VectorCase>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VectorCase {
    name: String,
    rates: Vec<RateRow>,
    pity: PityConfig,
    cost: f64,
    expected_composite: ExpComposite,
    expected_economics: ExpEconomics,
    #[serde(default)]
    simulate: Option<ExpSimulate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpComposite {
    base: Vec<f64>,
    gold_base: f64,
    comp: Vec<f64>,
    gold_comp: f64,
    cycle: f64,
    hard_prob: f64,
    up_rate: f64,
    up_share: f64,
    ev_comp: f64,
    degenerate: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SvFromV {
    kind: String,
    r: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpEconomics {
    sv: f64,
    sv_from: Option<SvFromV>,
    ev_new: f64,
    ev_end: f64,
    miss_rate: f64,
    shard_rate: f64,
    card_rate: f64,
    cost: f64,
    ratio_new: f64,
    ratio_end: f64,
    ratio_worst: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpSimulate {
    n: i64,
    seed: i64,
    gold: i64,
    hard_hits: i64,
    ten_win: i64,
    ten_tot: i64,
    max_gap: i64,
    counts: Vec<i64>,
    ev_sim: f64,
    ev_base: f64,
}

fn load() -> Vec<VectorCase> {
    serde_json::from_str::<VectorFile>(include_str!("gacha_math_vectors.json"))
        .expect("向量 JSON 解析失败")
        .cases
}

fn pool(c: &VectorCase) -> PoolConfig {
    PoolConfig {
        rates: c.rates.clone(),
        pity: c.pity.clone(),
    }
}

fn by_name<'a>(cases: &'a [VectorCase], n: &str) -> &'a VectorCase {
    let mut hit = cases.iter().filter(|c| c.name == n);
    let c = hit.next().unwrap_or_else(|| panic!("向量缺用例 {n}"));
    assert!(hit.next().is_none(), "向量重名 {n}");
    c
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * f64::max(1.0, b.abs())
}

#[test]
fn vectors_composite_mirror() {
    for c in load() {
        let co = composite(&pool(&c));
        let e = &c.expected_composite;
        assert_eq!(co.degenerate, e.degenerate, "{}: degenerate", c.name);
        assert!(near(co.gold_base, e.gold_base), "{}: gold_base", c.name);
        assert!(near(co.gold_comp, e.gold_comp), "{}: gold_comp", c.name);
        assert!(near(co.cycle, e.cycle), "{}: cycle", c.name);
        assert!(near(co.hard_prob, e.hard_prob), "{}: hard_prob", c.name);
        assert!(near(co.up_rate, e.up_rate), "{}: up_rate", c.name);
        assert!(near(co.up_share, e.up_share), "{}: up_share", c.name);
        assert!(near(co.ev_comp, e.ev_comp), "{}: ev_comp", c.name);
        assert_eq!(co.base.len(), e.base.len(), "{}: base.len", c.name);
        for (i, b) in e.base.iter().enumerate() {
            assert!(near(co.base[i], *b), "{}: base[{i}]", c.name);
        }
        assert_eq!(co.comp.len(), e.comp.len(), "{}: comp.len", c.name);
        for (i, b) in e.comp.iter().enumerate() {
            assert!(near(co.comp[i], *b), "{}: comp[{i}]", c.name);
        }
    }
}

#[test]
fn mass_conservation_sum_comp_is_one() {
    for c in load() {
        let co = composite(&pool(&c));
        let sum: f64 = co.comp.iter().sum();
        assert!((sum - 1.0).abs() <= 1e-9, "{}: Σ comp = {sum}", c.name);
    }
}

#[test]
fn vectors_economics_mirror() {
    for c in load() {
        let ec = economics(&pool(&c), c.cost);
        let e = &c.expected_economics;
        assert!(near(ec.sv, e.sv), "{}: sv", c.name);
        assert!(near(ec.ev_new, e.ev_new), "{}: ev_new", c.name);
        assert!(near(ec.ev_end, e.ev_end), "{}: ev_end", c.name);
        assert!(near(ec.miss_rate, e.miss_rate), "{}: miss_rate", c.name);
        assert!(near(ec.shard_rate, e.shard_rate), "{}: shard_rate", c.name);
        assert!(near(ec.card_rate, e.card_rate), "{}: card_rate", c.name);
        assert!(near(ec.ratio_new, e.ratio_new), "{}: ratio_new", c.name);
        assert!(near(ec.ratio_end, e.ratio_end), "{}: ratio_end", c.name);
        assert!(
            near(ec.ratio_worst, e.ratio_worst),
            "{}: ratio_worst",
            c.name
        );
        // 守卫口径恒取 max(新手, 毕业)
        assert_eq!(ec.ratio_worst, f64::max(ec.ratio_new, ec.ratio_end));
        match (&ec.sv_from, &e.sv_from) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                assert_eq!(a.kind, b.kind, "{}: sv_from.kind", c.name);
                assert_eq!(a.r, b.r, "{}: sv_from.r", c.name);
            }
            _ => panic!("{}: sv_from 一侧缺失", c.name),
        }
    }
}

#[test]
fn guard_backdoor_must_trip() {
    // 方案 §1.3 后门：压低合成价 ⇒ 碎片单价抬升 ⇒ ratioWorst > 100%。
    // G31-B 的运行时守卫（抽取前 >100% 直接 409）必须能被这个用例打红。
    let cases = load();
    let c = by_name(&cases, "backdoor_lr_synth_800");
    let ec = economics(&pool(c), c.cost);
    assert!(ec.ratio_worst > 1.0, "ratio_worst = {}", ec.ratio_worst);
    assert!(near(ec.sv, c.expected_economics.sv), "sv = {}", ec.sv);
}

#[test]
fn simulate_mirror_exact_integers() {
    // mulberry32 是纯整数流，两侧实现必须逐位同谱。
    let cases = load();
    let c = by_name(&cases, "mc_200k_seed42");
    let s = c.simulate.as_ref().expect("MC 用例缺失");
    let r = simulate(&pool(c), s.n, s.seed);
    assert_eq!(r.gold, s.gold, "gold");
    assert_eq!(r.hard_hits, s.hard_hits, "hard_hits");
    assert_eq!(r.ten_win, s.ten_win, "ten_win");
    assert_eq!(r.ten_tot, s.ten_tot, "ten_tot");
    assert_eq!(r.max_gap, s.max_gap, "max_gap");
    assert_eq!(r.counts, s.counts, "counts");
    assert!(near(r.ev_sim, s.ev_sim), "ev_sim");
}

#[test]
fn mc_within_sampling_error_of_dp() {
    let cases = load();
    let c = by_name(&cases, "mc_200k_seed42");
    let r = simulate(&pool(c), 200_000, 42);
    let dp = composite(&pool(c));
    let diff = (r.gold as f64 / r.n as f64 - dp.gold_comp).abs();
    assert!(diff < 0.005, "MC-DP 出金率差 {diff}");
}

#[test]
fn is_gold_threshold_is_three() {
    assert!(!is_gold("R"));
    assert!(!is_gold("SR"));
    assert!(is_gold("SSR"));
    assert!(is_gold("UR"));
    assert!(is_gold("LR"));
    assert!(!is_gold("SHARD"));
    assert!(!is_gold("MISS"));
}
