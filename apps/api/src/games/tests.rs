use super::*;

#[test]
fn bet_validation() {
    assert!(validate_bet(100).is_ok());
    assert!(validate_bet(0).is_err());
    assert!(validate_bet(-5).is_err());
    assert!(validate_bet(MAX_BET + 1).is_err());
}

/// 奖池已行表化（0242 池 / 0244 物品目录）。播种值的 EV 由迁移内 DO 块在落库时断言；
/// 这里锁的是**机制**：任何过 `validate_pool` 的池必然 EV<1，且坏配置各自被点名拒绝。
/// 价值口径：魔力位 = 票价×倍数；物品位 = arcade_items.anchor × 件数（绝不取登记价）。
#[test]
fn scratch_payout_is_exact_floor_of_bet_times_mult() {
    // 档位住在奖池行表里（0248），这里锁的是「倍率→派彩」这一件纯计算：
    // 100 注 × 千分倍率，只能是 0/50/100/200/1000 五档
    for mp in [0, 500, 1000, 2000, 10000] {
        let o = scratch_pay(100, mp);
        assert!(
            [0, 50, 100, 200, 1000].contains(&o.payout),
            "{mp} -> {:?}",
            o
        );
        assert!((o.multiplier - mp as f64 / 1000.0).abs() < 1e-9);
    }
    // 向下取整：1 注的 0.5 倍派 0（不是 0.5，也不是进位成 1）
    assert_eq!(scratch_pay(1, 500).payout, 0);
    assert_eq!(scratch_pay(999, 10000).payout, 9990);
}

#[test]
fn dice_regions_match_the_mechanic() {
    // 机制在代码（1-49 小 / 50-51 平 / 52-100 大），派彩在表：
    // 这里只锁「哪一区付」，它不能被配置改掉
    assert_eq!(outcome_side(1, Guess::Small), "win");
    assert_eq!(outcome_side(49, Guess::Small), "win");
    assert_eq!(outcome_side(50, Guess::Small), "tie");
    assert_eq!(outcome_side(51, Guess::Big), "tie");
    assert_eq!(outcome_side(52, Guess::Small), "lose");
    assert_eq!(outcome_side(100, Guess::Big), "win");
    for _ in 0..400 {
        assert!((1..=100).contains(&roll()));
    }
}

#[test]
fn bigsmall_table_must_cover_all_three_regions() {
    let e = |w: i64, wt: u32| PoolEntry {
        label: "x".into(),
        weight: wt,
        kind: EntryKind::Magic { mult_permille: w },
    };
    assert!(
        validate_bigsmall(&[], &[e(1000, 20)], &[e(0, 490)]).is_err(),
        "缺赢区必须拒"
    );
    assert!(
        validate_bigsmall(
            &[e(1900, 490)],
            &[e(1000, 20)],
            &[e(0, 290), e(0, 200)]
        )
        .is_ok(),
        "输区拆两档、合计仍是 490 —— 合法"
    );
    assert!(
        validate_bigsmall(&[e(1900, 490)], &[e(1000, 20)], &[e(0, 300)])
            .is_err(),
        "输区合计不等于 490 就是破坏 49/2/49"
    );
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

/// 刮刮乐播种表（0248 从设置键现值搬进 arcade_pools）的 EV 必须仍是 0.66，
/// 且权重铺满 100 —— 这条断言锁的是「搬家前后行为一字不差」。
#[test]
fn scratch_seeded_pool_expected_value_below_one() {
    let seeded = |w: &[u32], mp: &[i64]| -> Vec<PoolEntry> {
        w.iter()
            .zip(mp.iter())
            .map(|(wt, m)| PoolEntry {
                label: format!("{m}"),
                weight: *wt,
                kind: EntryKind::Magic { mult_permille: *m },
            })
            .collect()
    };
    let p = seeded(&[45, 30, 15, 8, 2], &[0, 500, 1000, 2000, 10000]);
    assert_eq!(p.iter().map(|e| e.weight).sum::<u32>(), 100);
    let ev = pool_ev(&p, 1);
    assert!((ev - 0.66).abs() < 1e-9, "EV 漂移: {ev}");
    validate_pool(&p, 1).expect("播种表必须过关");

    // 注额=票档时 EV 最高；注额越大，固定折算价的物品位被摊得越薄。
    // 所以闸门按票档（最低注额）算，是**对所有合法注额都成立**的那一侧。
    let hot = seeded(&[20, 30, 15, 20, 15], &[0, 500, 1000, 2000, 10000]);
    assert!(
        matches!(
            validate_pool(&hot, 1),
            Err(PoolError::ExpectedValueNotBelowOne(_))
        ),
        "EV>=1 必须被拒: {:?}",
        pool_ev(&hot, 1)
    );
    // 同一张表在 100 票档下 EV 只有物品位摊薄后的部分 —— 说明票档是真闸门
    let with_item = vec![
        PoolEntry {
            label: "空".into(),
            weight: 50,
            kind: EntryKind::Magic { mult_permille: 0 },
        },
        PoolEntry {
            label: "券".into(),
            weight: 50,
            kind: EntryKind::Item {
                item_key: "ticket".into(),
                qty: 1,
                anchor: 900,
            },
        },
    ];
    assert!(
        validate_pool(&with_item, 100).is_err(),
        "100 票档下 900 券占一半必破 1"
    );
    assert!(pool_ev(&with_item, 10000) < 1.0, "注额摊薄后才可能合法");
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
