//! 斗地主领域层单测（从 doudizhu.rs 拆出：主文件贴 300 行门禁）。

use super::*;

use super::*;

fn c(rank: u8, suit: usize) -> u8 {
    if rank >= 13 {
        52 + (rank - 13)
    } else {
        rank * 4 + suit as u8
    }
}

fn bomb_of(rank: u8) -> Combo {
    let cards: Vec<u8> = (0..4).map(|s| c(rank, s)).collect();
    detect(&cards).unwrap()
}

#[test]
fn rank_order_and_labels() {
    assert_eq!(Rank::of(c(0, 0)), Rank(0));
    assert_eq!(Rank::of(c(12, 3)), Rank(12));
    assert_eq!(Rank::of(52), Rank(13));
    assert_eq!(Rank::of(53), Rank(14));
    assert!(Rank(14) > Rank(12));
    assert_eq!(Rank(12).label(), "2");
    assert_eq!(Rank(13).label(), "w");
}

#[test]
fn deal_shape_and_no_overlap() {
    let (hands, kitty) = deal();
    let mut all = Vec::new();
    for h in &hands {
        all.extend_from_slice(h);
    }
    all.extend_from_slice(&kitty);
    assert_eq!(all.len(), 54);
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 54, "发牌有重复");
    // 每手内降序
    for h in &hands {
        assert!(h.windows(2).all(|w| Rank::of(w[0]) >= Rank::of(w[1])));
    }
}

#[test]
fn basic_kinds() {
    assert_eq!(detect(&[c(0, 0)]).unwrap().kind, ComboKind::Single);
    assert_eq!(detect(&[c(3, 0), c(3, 1)]).unwrap().kind, ComboKind::Pair);
    assert_eq!(
        detect(&[c(3, 0), c(3, 1), c(3, 2)]).unwrap().kind,
        ComboKind::Triple
    );
    assert_eq!(
        detect(&[c(3, 0), c(3, 1), c(3, 2), c(3, 3)]).unwrap().kind,
        ComboKind::Bomb
    );
    assert_eq!(detect(&[52, 53]).unwrap().kind, ComboKind::Rocket);
}

#[test]
fn triple_with_attachments() {
    let ts = [c(5, 0), c(5, 1), c(5, 2), c(0, 3)];
    assert_eq!(detect(&ts).unwrap().kind, ComboKind::TripleSingle);
    let tp = [c(5, 0), c(5, 1), c(5, 2), c(0, 0), c(0, 1)];
    assert_eq!(detect(&tp).unwrap().kind, ComboKind::TriplePair);
    // 三带三张不合法（带牌必须 1 张或 1 对）
    let t3 = [c(5, 0), c(5, 1), c(5, 2), c(0, 0), c(0, 1), c(0, 2)];
    assert!(detect(&t3).is_none());
}

#[test]
fn straights_exclude_two_and_jokers() {
    // 34567 合法
    let s: Vec<u8> = (0..5u8).map(|r| c(r, (r % 4) as usize)).collect();
    let k = detect(&s).unwrap();
    assert_eq!(k.kind, ComboKind::Straight);
    assert_eq!(k.major, Rank(4));
    // 含 2 的顺子不合法（10JQKA2 断）
    let bad: Vec<u8> = (8..13u8).map(|r| c(r, (r % 4) as usize)).collect();
    assert!(detect(&bad).is_none(), "10JQKA2 不是顺子");
    // A2345 不合法（2 是顶点不能绕回 3）
    let wrap = [c(11, 0), c(12, 0), c(0, 0), c(1, 0), c(2, 0)].to_vec();
    assert!(detect(&wrap).is_none());
    // 王不能进顺子
    let mut joker: Vec<u8> = (0..4u8).map(|r| c(r, (r % 4) as usize)).collect();
    joker.push(52);
    assert!(detect(&joker).is_none());
}

#[test]
fn pair_straight_min_three() {
    let p: Vec<u8> = (0..3u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect();
    assert_eq!(detect(&p).unwrap().kind, ComboKind::PairStraight);
    // 两对不算连对
    let two: Vec<u8> = (0..2u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect();
    assert!(detect(&two).is_none());
}

#[test]
fn planes() {
    // 333444 纯飞机
    let p: Vec<u8> = [
        (0..3).map(|s| c(0, s)).collect::<Vec<_>>(),
        (0..3).map(|s| c(1, s)).collect::<Vec<_>>(),
    ]
    .concat();
    assert_eq!(detect(&p).unwrap().kind, ComboKind::Plane);
    // 333444 + 两张单 = 飞机带单（带数必须=机身组数）
    let p2: Vec<u8> = [p.clone(), vec![c(2, 0), c(9, 0)]].concat();
    let d = detect(&p2).unwrap();
    assert_eq!(d.kind, ComboKind::PlaneSingle);
    // 333444 + 一对一单 ≠ 合法（带牌形状不齐）
    let bad_wing: Vec<u8> =
        [p.clone(), vec![c(2, 0), c(2, 1), c(9, 0)]].concat();
    assert!(detect(&bad_wing).is_none());
    // 带对：3334445566
    let pp: Vec<u8> = [
        p.clone(),
        (4..6u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect(),
    ]
    .concat();
    assert_eq!(detect(&pp).unwrap().kind, ComboKind::PlanePair);
    // 含 2 的飞机不合法
    let p2s: Vec<u8> = [
        (0..3).map(|s| c(12, s)).collect::<Vec<_>>(),
        (0..3).map(|s| c(11, s)).collect::<Vec<_>>(),
    ]
    .concat();
    assert!(detect(&p2s).is_none());
}

#[test]
fn plane_pair_with_adjacent_pair_wing() {
    // 3334445566：机身 333444，翅膀 55 66——与机身相邻的 44 也凑成对，
    // 全局 pairs 计数会误拒此标准牌型（资深玩家实测抓到）。
    let pp: Vec<u8> = [
        (0..3).map(|s| c(0, s)).collect::<Vec<_>>(),
        (0..3).map(|s| c(1, s)).collect::<Vec<_>>(),
        (4..6u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect(),
    ]
    .concat();
    let d = detect(&pp).unwrap();
    assert_eq!(d.kind, ComboKind::PlanePair);
    assert_eq!(d.major, Rank(1));
}

#[test]
fn four_with_two() {
    // 4带2单
    let f4: Vec<u8> = [
        (0..4).map(|s| c(7, s)).collect::<Vec<_>>(),
        vec![c(2, 0), c(3, 1)],
    ]
    .concat();
    assert_eq!(detect(&f4).unwrap().kind, ComboKind::FourTwoSingle);
    // 4带两对
    let f4p: Vec<u8> = [
        (0..4).map(|s| c(7, s)).collect::<Vec<_>>(),
        (1..3u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect(),
    ]
    .concat();
    assert_eq!(detect(&f4p).unwrap().kind, ComboKind::FourTwoPair);
}

#[test]
fn beats_matrix() {
    let s3 = detect(&[c(0, 0)]).unwrap();
    let s5 = detect(&[c(2, 1)]).unwrap();
    let pair = detect(&[c(2, 0), c(2, 1)]).unwrap();
    let bomb = bomb_of(2);
    let rocket = detect(&[52, 53]).unwrap();
    // 同型比大小
    assert!(beats(&s5, Some(&s3)));
    assert!(!beats(&s3, Some(&s5)));
    // 异型不可压
    assert!(!beats(&pair, Some(&s5)));
    // 炸弹压一切非炸弹；王炸压炸弹；大炸弹压小炸弹
    assert!(beats(&bomb, Some(&s5)));
    assert!(beats(&rocket, Some(&bomb)));
    let big_bomb = bomb_of(5);
    assert!(beats(&big_bomb, Some(&bomb)));
    assert!(!beats(&bomb, Some(&big_bomb)));
    // 领出随便出
    assert!(beats(&s3, None));
}

#[test]
fn legal_pick_rejects_foreign_cards() {
    let hand = vec![c(0, 0), c(1, 1), c(1, 2)];
    assert!(legal_pick(&hand, &[c(9, 0)], None).is_none());
    assert!(legal_pick(&hand, &[c(0, 0)], None).is_some());
}

#[test]
fn multiplier_caps_at_eight() {
    assert_eq!(multiplier_of(1, 0, false), 1);
    assert_eq!(multiplier_of(3, 0, false), 3);
    assert_eq!(multiplier_of(1, 1, false), 2);
    assert_eq!(multiplier_of(2, 2, false), 8);
    // 3×2^2×2 = 24 → 封顶 8
    assert_eq!(multiplier_of(3, 2, true), 8);
    assert_eq!(multiplier_of(3, 3, true), 8);
}

#[test]
fn settle_math() {
    // 回款 = 本金 + 抽水后净收益；本金不参与抽水。
    // 农民赢：100 + 100×1×2×900/1000 = 280
    assert_eq!(settle_payout(Role::Farmer, 100, 2, 100), 280);
    // 地主赢：100 + 100×2×2×900/1000 = 460
    assert_eq!(settle_payout(Role::Landlord, 100, 2, 100), 460);
    // 抽水 0：100 + 200 = 300
    assert_eq!(settle_payout(Role::Farmer, 100, 2, 0), 300);
    // 抽水钳硬上限 500
    assert_eq!(settle_payout(Role::Farmer, 100, 2, 999), 200);
    // 关键回归：农民赢倍数 1，旧版回款 90 净 -10（赢反而亏）；
    // 新版回款 190、net = 90 ≥ 0。
    assert_eq!(settle_payout(Role::Farmer, 100, 1, 100), 190);
    assert!(settle_payout(Role::Farmer, 100, 1, 100) - 100 >= 0);
    // 小注：100 本金保底 + 净收益 0（900/1000 截断）
    assert_eq!(settle_payout(Role::Farmer, 1, 1, 100), 1);
}

#[test]
fn ev_stays_below_one_with_default_rake() {
    let ev = doudizhu_expected_value(DOU_DIZHU_RAKE);
    assert!(ev < 1.0, "斗地主 EV 必须 < 1（回收口红线），实得 {ev}");
    // 抽水钳硬上限后仍 < 1
    let ev_cap = doudizhu_expected_value(RAKE_PERMILLE_HARD_CAP);
    assert!(ev_cap < 1.0, "抽水上限档 EV 必须 < 1，实得 {ev_cap}");
}

/// 派彩键前缀守卫：战绩口 `/games/rounds` 按 `game-doudizhu-win:` JOIN。
/// 显式点名：`use super::*` 同时带进 games.rs 重导出的 bs_room::win_idem。
#[test]
fn win_idem_matches_rounds_join() {
    let win = doudizhu_settle::win_idem("game-doudizhu:42:abc");
    assert_eq!(win, "game-doudizhu-win:game-doudizhu:42:abc");
    assert!(win.starts_with("game-doudizhu-win:"));
    // bs_room 的同名函数前缀不同，不可混用
    let bs = win_idem("game-doudizhu:42:abc");
    assert_ne!(bs, win);
}
