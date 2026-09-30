//! 奖池校验单测：锁的是**机制**而不是某张表。
//!
//! 奖池已行表化（0242 池 / 0244 物品目录），播种值的 EV 由迁移内 DO 块在落库时
//! 断言；这里只保证「任何过 validate_pool 的池必然 EV<1」以及坏配置各自被点名拒绝。

use super::*;
const TICKET: i64 = 100;

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

/// 与 0242 播种同形的夹具（测试不连库；运行时的唯一清单只有表）
fn seeded_pool() -> Vec<PoolEntry> {
    vec![
        magic("谢谢参与", 731, 0),
        magic("再来一次", 120, 1_000),
        magic("2x 魔力", 60, 2_000),
        magic("3x 魔力", 50, 3_000),
        magic("5x 魔力", 25, 5_000),
        magic("10x 魔力", 10, 10_000),
        magic("20x 魔力", 3, 20_000),
        magic("50x 魔力", 1, 50_000),
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
    assert!(
        (pool_ev(&p, TICKET) - 0.09).abs() < 1e-9,
        "物品位 EV: {}",
        pool_ev(&p, TICKET)
    );
}

#[test]
fn pool_gate_rejects_every_backdoor_by_name() {
    // ① 抬经济奖权重 -> EV 破 1
    let mut p = seeded_pool();
    p[7].weight = 40;
    assert!(matches!(
        validate_pool(&p, TICKET),
        Err(PoolError::ExpectedValueNotBelowOne(_))
    ));
    // ② 抬倍数
    let mut p = seeded_pool();
    p[7].kind = EntryKind::Magic {
        mult_permille: 900_000,
    };
    assert!(matches!(
        validate_pool(&p, TICKET),
        Err(PoolError::ExpectedValueNotBelowOne(_))
    ));
    // ③ 空池
    assert_eq!(validate_pool(&[], TICKET), Err(PoolError::Empty));
    // ④ 零权重档
    let mut p = seeded_pool();
    p[2].weight = 0;
    assert_eq!(
        validate_pool(&p, TICKET),
        Err(PoolError::ZeroWeight("2x 魔力".into()))
    );
    // ⑤ 中性池 EV 恰为 1 也拒
    assert!(matches!(
        validate_pool(&[magic("a", 1, 2_000)], TICKET),
        Err(PoolError::ExpectedValueNotBelowOne(_))
    ));
    // ⑥ 物品位引用目录里不存在/停用/无折算价的物品 —— anchor<=0 即空头承诺
    assert_eq!(
        validate_pool(
            &[magic("b", 999, 0), item("空头券", 1, "gone", 1, 0)],
            TICKET
        ),
        Err(PoolError::UnknownItem("gone".into()))
    );
    // ⑦ 票价非正：EV 的分母，0 会让「返还率」变成除零
    assert_eq!(
        validate_pool(&seeded_pool(), 0),
        Err(PoolError::BadTicket(0))
    );
}

#[test]
fn pool_ev_is_invariant_to_weight_scaling() {
    // 同比例放大权重不改变 EV —— 防「加一档稀释」被当成降 EV
    let a = seeded_pool();
    let b: Vec<PoolEntry> = a
        .iter()
        .map(|e| magic(&e.label, e.weight * 7, m(e)))
        .collect();
    assert!((pool_ev(&a, TICKET) - pool_ev(&b, TICKET)).abs() < 1e-9);
}

fn m(e: &PoolEntry) -> i64 {
    match &e.kind {
        EntryKind::Magic { mult_permille } => *mult_permille,
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

// ---- 票档 / 最低注额与单次下注上限 ----

#[test]
fn ticket_above_max_bet_is_refused() {
    // 票价 10000 而 games_max_bet 是 500：低于票档拒开、高于上限也拒开，
    // 两个规则夹住所有注额 —— 这张表存进去，玩法就不再可玩
    assert!(matches!(
        validate_ticket_cap(10_000, 500),
        Err(PoolError::TicketAboveMaxBet { .. })
    ));
    // 正好等于上限合法（注额只有一个值，但它是合法的）
    assert!(validate_ticket_cap(500, 500).is_ok());
    assert!(validate_ticket_cap(100, 500).is_ok());
    // 上限被设成 0 视为「不额外约束」，不拿它当拒开一切的理由
    assert!(validate_ticket_cap(100, 0).is_ok());
    // 票价非正是另一件事，报另一个错
    assert!(matches!(
        validate_ticket_cap(0, 500),
        Err(PoolError::BadTicketValue(0))
    ));
}
