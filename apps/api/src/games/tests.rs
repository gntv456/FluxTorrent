use super::*;

#[test]
fn bet_validation() {
    assert!(validate_bet(100).is_ok());
    assert!(validate_bet(0).is_err());
    assert!(validate_bet(-5).is_err());
    assert!(validate_bet(MAX_BET + 1).is_err());
}

/// 奖池已行表化（0242），播种值的 EV=0.725 由迁移内的 DO 块在**落库时**断言。
/// 这里锁的是机制而非某张表：任何过 `validate_pool` 的池必然 EV<1，
/// 且四类坏配置必须各自被点名拒绝 —— 不会红的门禁不算门禁。
fn prize(label: &str, weight: u32, payout: i64) -> JggPrize {
    JggPrize { label: label.to_string(), weight, payout }
}

/// 与 0242 播种同形的夹具（测试不连库；运行时的唯一清单只有表）
fn seeded_pool() -> Vec<JggPrize> {
    vec![
        prize("谢谢参与", 731, 0),
        prize("再来一次", 120, 1),
        prize("2x 魔力", 60, 2),
        prize("3x 魔力", 50, 3),
        prize("5x 魔力", 25, 5),
        prize("10x 魔力", 10, 10),
        prize("20x 魔力", 3, 20),
        prize("50x 魔力", 1, 50),
    ]
}

#[test]
fn jgg_seeded_pool_is_valid_and_recovers() {
    let pool = seeded_pool();
    validate_pool(&pool).expect("播种奖池必须合法");
    let ev = jgg_ev(&pool);
    assert!((ev - 0.725).abs() < 1e-9, "EV 漂移: {ev}");
    assert!(ev < 1.0, "EV>=1 就不是回收口");
}

#[test]
fn jgg_gate_rejects_the_five_backdoors() {
    // ① 抬经济奖权重：把 50x 从 1 抬到 40 -> EV 破 1
    let mut p = seeded_pool();
    p[7].weight = 40;
    assert!(matches!(validate_pool(&p), Err(PoolError::ExpectedValueNotBelowOne(_))));
    // ② 抬赔率倍数：50x 改 900
    let mut p = seeded_pool();
    p[7].payout = 900;
    assert!(matches!(validate_pool(&p), Err(PoolError::ExpectedValueNotBelowOne(_))));
    // ③ 空池（全部停用 / 清单没渲染出来）
    assert_eq!(validate_pool(&[]), Err(PoolError::Empty));
    // ④ 权重为 0 的档位：配了但永远抽不到
    let mut p = seeded_pool();
    p[2].weight = 0;
    assert_eq!(validate_pool(&p), Err(PoolError::ZeroWeight("2x 魔力".into())));
    // ⑤ 中性池 EV 恰为 1 也必须拒（不允许「不赚不亏」的玩法）
    let neutral = vec![prize("a", 1, 2)];
    assert!(matches!(validate_pool(&neutral), Err(PoolError::ExpectedValueNotBelowOne(_))));
}

#[test]
fn jgg_ev_formula_is_weight_scale_invariant() {
    // 同比例放大权重不改变 EV —— EV 只看比例，防「加一档稀释」被当成降 EV
    let a = seeded_pool();
    let b: Vec<JggPrize> = a.iter().map(|p| prize(&p.label, p.weight * 7, p.payout)).collect();
    assert!((jgg_ev(&a) - jgg_ev(&b)).abs() < 1e-9);
}

#[test]
fn scratch_payout_bounds() {
    for _ in 0..1000 {
        let o = scratch_play(100);
        // 派彩只能是 0/50/100/200/1000 五档
        assert!([0, 50, 100, 200, 1000].contains(&o.payout));
    }
}

#[test]
fn dice_tie_returns_stake() {
    // 平局区 50/51 返本：强行构造（函数随机，验证结构）
    for _ in 0..500 {
        let o = guess_play(100, Guess::Big);
        if (50..=51).contains(&o.number) {
            assert_eq!(o.payout, 100);
            assert!(!o.player_win);
        }
        if o.player_win {
            assert_eq!(o.payout, 190);
        }
    }
}

/// 经济纪律（2026-09-19 产品决策）：娱乐玩法一律回收，期望回报必须 < 1。
/// 猜大小赢面 49% / 平 2% / 输 49%，赔率 ≥ 2.0 时 EV ≥ 1.0 且可双向零风险对冲 ——
/// 这个断言锁死「赔率 < 2.0」，调整赔率必须同步复算本式与经济文档。
#[test]
fn bigsmall_expected_value_below_one() {
    let ev = bigsmall_expected_value(BIGSMALL_WIN_MULT_PERMILLE);
    assert!(ev < 1.0, "猜大小 EV 未回收: {ev}");
    assert!(
        (ev - 0.951).abs() < 1e-9,
        "EV 漂移: {ev}（1.9x 应为 0.951）"
    );
    assert!(
        BIGSMALL_WIN_MULT_PERMILLE < 2000,
        "赔率必须 < 2000‰，否则 EV ≥ 1 且可对冲套零风险"
    );
    // 隐含：赔率越高 EV 越高，2.0 处恰好回到 1.0（不允许）
    assert!((bigsmall_expected_value(2000) - 1.0).abs() < 1e-9);
}

/// 刮刮乐档位参数化的期望回报仍 < 1（四档可配，10x 取余数）
#[test]
fn scratch_expected_value_below_one() {
    let ev = |o: &ScratchOdds| {
        (o.empty as f64 * 0.0
            + o.half as f64 * 0.5
            + o.one as f64 * 1.0
            + o.two as f64 * 2.0
            + o.ten as f64 * 10.0)
            / 100.0
    };
    let d = ScratchOdds::DEFAULT;
    assert_eq!(
        d.empty + d.half + d.one + d.two + d.ten,
        100,
        "档位必须铺满 100%"
    );
    assert!((ev(&d) - 0.66).abs() < 1e-9, "缺省 EV 漂移: {}", ev(&d));
    // 争议配置（前四档吃满）必须回落缺省，不能构造出必中/增发档位
    let bad = ScratchOdds::from_parts(60, 30, 15, 8, 2);
    assert_eq!(bad.empty + bad.half + bad.one + bad.two + bad.ten, 100);
    assert!(ev(&bad) < 1.0, "越界配置未回落，EV={}", ev(&bad));
    // 10x 档留 0（或与前四档合计不为 100）→ 按余数推导，总量恒 100
    let auto = ScratchOdds::from_parts(50, 30, 15, 3, 0);
    assert_eq!(auto.empty + auto.half + auto.one + auto.two + auto.ten, 100);
    assert_eq!(auto.ten, 2, "余数应为 2");
    // 站长显式配置 10x 且合计正好 100 → 采用配置值
    let explicit = ScratchOdds::from_parts(50, 30, 15, 3, 2);
    assert_eq!(explicit.ten, 2);
    assert_eq!(
        explicit.empty
            + explicit.half
            + explicit.one
            + explicit.two
            + explicit.ten,
        100
    );
}

/// 九宫格头奖 50x 的 EV 已由 jgg_expected_value_house_edge 锁定（0.725）。

#[test]
fn market_price_stable_in_window() {
    let w = market_window_start(1788000000);
    assert_eq!(market_price(100, w), market_price(100, w));
    // 相邻窗口价格可以变也可以不变，但窗口起点对齐 4h
    assert_eq!(w % (4 * 3600), 0);
}

#[test]
fn market_price_within_bounds() {
    for w in 0..50 {
        let p = market_price(1000, w * 14400);
        assert!((500..=1500).contains(&p), "price {p} out of ±50% bounds");
    }
}

/// 审计修复（P1 套利）回归锁：买种侧与收获侧因子必须错开——
/// 若两侧同因子，存在窗口对 (w_buy, w_sell) 使 seed 价 ×0.5 而 yield 价 ×1.5，
/// 确定性利润 +200%。锁「同窗口下两侧因子不同的窗口占比 ≥ 80%」，
/// 防止未来改哈希时不慎回到同因子。
#[test]
fn buy_and_harvest_factors_are_independent() {
    let mut diff = 0;
    let n = 200;
    for w in 0..n {
        let buy = market_price(10_000, w * 14400);
        let sell = harvest_market_price(10_000, w * 14400);
        if buy != sell {
            diff += 1;
        }
    }
    assert!(
        (diff as f64 / n as f64) >= 0.8,
        "buy/harvest factors too correlated: {diff}/{n}"
    );
}

#[test]
fn jgg_weights_cover_all() {
    let pool = seeded_pool();
    let total: u64 = pool.iter().map(|p| u64::from(p.weight)).sum();
    assert_eq!(total, 1000);
    // 档位数从池子来，别硬编码：奖池行表化之后「8」随时会变，
    // 写死 [false; 8] 的话档数一变就是 panic 而不是测试失败。
    // 抽样次数按最低权重动态推算，期望最稀有档被抽到 ~25 次，漏检概率约 e^-25。
    let min_weight = pool.iter().map(|p| p.weight).min().unwrap();
    assert!(min_weight > 0, "存在权重为 0 的档位");
    let draws = 25 * (total as usize) / (min_weight as usize);
    let mut seen = vec![false; pool.len()];
    for _ in 0..draws {
        let d = jgg_draw_from(&pool).expect("合法池必须可抽样");
        seen[d.index] = true;
    }
    assert!(seen.iter().all(|&s| s), "some prize never drawn: {seen:?}");
}

#[test]
fn jgg_draw_refuses_invalid_pool_shape() {
    // 空池 / 权重合计 0：不猜一档出来，而是 None 让调用方关闸
    assert!(jgg_draw_from(&[]).is_none());
    assert!(jgg_draw_from(&[prize("zero", 0, 1)]).is_none());
}
