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
const TICKET: i64 = 100;

fn magic(label: &str, weight: u32, multiples: i64) -> PoolEntry {
    PoolEntry { label: label.to_string(), weight, kind: EntryKind::Magic { multiples } }
}

fn item(label: &str, weight: u32, key: &str, qty: i32, anchor: i64) -> PoolEntry {
    PoolEntry {
        label: label.to_string(),
        weight,
        kind: EntryKind::Item { item_key: key.to_string(), qty, anchor },
    }
}

/// 与 0242 播种同形的夹具（测试不连库；运行时的唯一清单只有表）
fn seeded_pool() -> Vec<PoolEntry> {
    vec![
        magic("谢谢参与", 731, 0),
        magic("再来一次", 120, 1),
        magic("2x 魔力", 60, 2),
        magic("3x 魔力", 50, 3),
        magic("5x 魔力", 25, 5),
        magic("10x 魔力", 10, 10),
        magic("20x 魔力", 3, 20),
        magic("50x 魔力", 1, 50),
    ]
}

#[test]
fn pool_seeded_shape_is_valid_and_recovers() {
    let p = seeded_pool();
    validate_pool(&p, TICKET).expect("播种奖池必须合法");
    let ev = pool_ev(&p, TICKET);
    assert!((ev - 0.725).abs() < 1e-9, "EV 漂移: {ev}");
    assert!(ev < 1.0, "EV>=1 就不是回收口");
}

#[test]
fn pool_item_entry_is_valued_by_anchor_not_price() {
    // 一张 anchor=900 的抽卡券，权重 10/1000：它给玩家的等值就是 900
    let p = vec![
        magic("谢谢参与", 990, 0),
        item("抽卡券", 10, "ticket", 1, 900),
    ];
    validate_pool(&p, TICKET).expect("合法");
    // Σ(权重×价值)/Σ权重/票价 = (10×900)/1000/100 = 0.09
    assert!((pool_ev(&p, TICKET) - 0.09).abs() < 1e-9, "物品位 EV: {}", pool_ev(&p, TICKET));
}

#[test]
fn pool_gate_rejects_every_backdoor_by_name() {
    // ① 抬经济奖权重 -> EV 破 1
    let mut p = seeded_pool();
    p[7].weight = 40;
    assert!(matches!(validate_pool(&p, TICKET), Err(PoolError::ExpectedValueNotBelowOne(_))));
    // ② 抬倍数
    let mut p = seeded_pool();
    p[7].kind = EntryKind::Magic { multiples: 900 };
    assert!(matches!(validate_pool(&p, TICKET), Err(PoolError::ExpectedValueNotBelowOne(_))));
    // ③ 空池
    assert_eq!(validate_pool(&[], TICKET), Err(PoolError::Empty));
    // ④ 零权重档
    let mut p = seeded_pool();
    p[2].weight = 0;
    assert_eq!(validate_pool(&p, TICKET), Err(PoolError::ZeroWeight("2x 魔力".into())));
    // ⑤ 中性池 EV 恰为 1 也拒
    assert!(matches!(validate_pool(&[magic("a", 1, 2)], TICKET),
                     Err(PoolError::ExpectedValueNotBelowOne(_))));
    // ⑥ 物品位引用目录里不存在/停用/无折算价的物品 —— anchor<=0 即空头承诺
    assert_eq!(
        validate_pool(&[magic("b", 999, 0), item("空头券", 1, "gone", 1, 0)], TICKET),
        Err(PoolError::UnknownItem("gone".into()))
    );
    // ⑦ 票价非正：EV 的分母，0 会让「返还率」变成除零
    assert_eq!(validate_pool(&seeded_pool(), 0), Err(PoolError::BadTicket(0)));
}

#[test]
fn pool_ev_is_invariant_to_weight_scaling() {
    // 同比例放大权重不改变 EV —— 防「加一档稀释」被当成降 EV
    let a = seeded_pool();
    let b: Vec<PoolEntry> = a.iter().map(|e| magic(&e.label, e.weight * 7, m(e))).collect();
    assert!((pool_ev(&a, TICKET) - pool_ev(&b, TICKET)).abs() < 1e-9);
}

fn m(e: &PoolEntry) -> i64 {
    match &e.kind {
        EntryKind::Magic { multiples } => *multiples,
        EntryKind::Item { .. } => 0,
    }
}

#[test]
fn pool_item_value_floors_at_fallback() {
    // 物品即使发不出去，站点也已欠一笔回落价；EV 取两者较大侧才算保守
    let cheap = item("廉价外观", 1, "frame", 1, 5);
    assert_eq!(cheap.value(TICKET), (TICKET * FALLBACK_MULT).max(5));
    let zero = item("零价值", 1, "frame", 1, 0);
    assert_eq!(zero.value(TICKET), TICKET * FALLBACK_MULT);
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
    // 前四档吃满 / EV 破 1 的坏配置一律 Err —— **不再回落缺省**：
    // 回落会让站长以为改生效了，而实际跑的是另一张表（静默后门）。
    let over = ScratchOdds::try_from_parts(60, 30, 15, 8, 2);
    assert!(over.is_err(), "前四档合计 113 >= 100 必须被拒");
    let hot = ScratchOdds::try_from_parts(20, 30, 15, 20, 15);
    assert!(matches!(hot, Err(ref m) if m.contains("增发")), "EV>=1 必须被拒: {:?}", hot);
    // 10x 档留 0（或与前四档合计不为 100）→ 按余数推导，总量恒 100。
    // 这条保留：它是文档化的「只配前三档」设计，不是偷改玩家看得见的赔率。
    let auto = ScratchOdds::try_from_parts(50, 30, 15, 3, 0).expect("合法");
    assert_eq!(auto.empty + auto.half + auto.one + auto.two + auto.ten, 100);
    assert_eq!(auto.ten, 2, "余数应为 2");
    // 站长显式配置 10x 且合计正好 100 → 采用配置值
    let explicit = ScratchOdds::try_from_parts(50, 30, 15, 3, 2).expect("合法");
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
fn pool_weights_cover_all() {
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
        let d = draw_entry(&pool).expect("合法池必须可抽样");
        seen[d.index] = true;
    }
    assert!(seen.iter().all(|&s| s), "some prize never drawn: {seen:?}");
}

#[test]
fn draw_refuses_invalid_pool_shape() {
    // 空池 / 权重合计 0：不猜一档出来，而是 None 让调用方关闸
    assert!(draw_entry(&[]).is_none());
    assert!(draw_entry(&[magic("zero", 0, 1)]).is_none());
}
