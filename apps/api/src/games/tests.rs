use super::*;

#[test]
fn bet_validation() {
    assert!(validate_bet(100).is_ok());
    assert!(validate_bet(0).is_err());
    assert!(validate_bet(-5).is_err());
    assert!(validate_bet(MAX_BET + 1).is_err());
}

/// 九宫格期望回报恒为 0.725（庄家优势 27.5%）——防止赔率表回归到增发配置
#[test]
fn jgg_expected_value_house_edge() {
    let total: u32 = JGG_PRIZES.iter().map(|p| p.weight).sum();
    assert_eq!(total, 1000, "权重总和建议恒为 1000");
    let ev: f64 = JGG_PRIZES
        .iter()
        .map(|p| p.weight as f64 * p.payout as f64)
        .sum::<f64>()
        / total as f64;
    assert!(
        (ev - 0.725).abs() < 1e-9,
        "EV 漂移: {ev}（调整赔率表必须同步更新本断言与经济模型）"
    );
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
    let total: u32 = JGG_PRIZES.iter().map(|p| p.weight).sum();
    assert_eq!(total, 1000);
    // 抽样次数按最低权重动态推算：期望最稀有档被抽到 ~25 次，
    // 漏检概率 ≈ e^-25。不能用固定次数——赔率表调整会改变最低权重
    // （旧表最低 5/1000 固定 2000 次即可，EV 修复后 50x 降至 1/1000，
    // 2000 次漏检概率高达 ~13%，测试随机失败）。
    let min_weight = JGG_PRIZES.iter().map(|p| p.weight).min().unwrap();
    assert!(min_weight > 0, "存在权重为 0 的档位，该奖永远不可能被抽到");
    let draws = 25 * total / min_weight;
    let mut seen = [false; 8];
    for _ in 0..draws {
        let d = jgg_draw();
        seen[d.index] = true;
    }
    assert!(seen.iter().all(|&s| s), "some prize never drawn: {seen:?}");
}
