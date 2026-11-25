//! 经典规则回归：资深玩家视角的边界牌型与压制口径。
//! 早期用 println 探针发现，固化为断言防回归。
use crate::games::doudizhu::{beats, detect, ComboKind, Rank};
use crate::games::doudizhu_ai::ai_choose;

fn c(rank: u8, suit: usize) -> u8 {
    if rank >= 13 {
        52 + (rank - 13)
    } else {
        rank * 4 + suit as u8
    }
}
fn kind(cards: &[u8]) -> Option<ComboKind> {
    detect(cards).map(|c| c.kind)
}

#[test]
fn classic_straight_boundaries() {
    // rank: 0=3 ... 7=10, 8=J, 9=Q, 10=K, 11=A, 12=2
    let ten_ace = [c(7, 0), c(8, 0), c(9, 0), c(10, 0), c(11, 0)];
    assert_eq!(kind(&ten_ace), Some(ComboKind::Straight));
    let jqka2 = [c(8, 0), c(9, 0), c(10, 0), c(11, 0), c(12, 0)];
    assert!(kind(&jqka2).is_none(), "JQKA2 不是顺子");
    let gap = [c(6, 0), c(8, 0), c(9, 0), c(10, 0), c(11, 0)];
    assert!(kind(&gap).is_none(), "缺牌不连续");
    let seven: Vec<u8> = (0..7u8).map(|r| c(r, 0)).collect();
    assert_eq!(kind(&seven), Some(ComboKind::Straight));
    // 连对仅两张 KKAA 不成立（需 ≥3 对）
    let two_pairs = [c(10, 0), c(10, 1), c(11, 0), c(11, 1)];
    assert!(kind(&two_pairs).is_none());
}

#[test]
fn classic_planes_and_four_with_two() {
    let plane3 = [
        c(11, 0),
        c(11, 1),
        c(11, 2),
        c(10, 0),
        c(10, 1),
        c(10, 2),
        c(9, 0),
        c(9, 1),
        c(9, 2),
    ];
    assert_eq!(kind(&plane3), Some(ComboKind::Plane));
    let plane_wing = [
        c(3, 0),
        c(3, 1),
        c(3, 2),
        c(4, 0),
        c(4, 1),
        c(4, 2),
        c(5, 0),
        c(6, 0),
    ];
    assert_eq!(kind(&plane_wing), Some(ComboKind::PlaneSingle));
    // 三带一可用王当翅膀
    let triple_joker = [c(3, 0), c(3, 1), c(3, 2), 52];
    assert_eq!(kind(&triple_joker), Some(ComboKind::TripleSingle));
    // 四带二可用双王
    let four_jokers = [c(3, 0), c(3, 1), c(3, 2), c(3, 3), 52, 53];
    assert_eq!(kind(&four_jokers), Some(ComboKind::FourTwoSingle));
    // 四带二不是炸弹，压不了单张
    let four_two = detect(&four_jokers).unwrap();
    let single = detect(&[c(9, 0)]).unwrap();
    assert!(!beats(&four_two, Some(&single)));
}

#[test]
fn rocket_beats_all_bombs() {
    let rocket = detect(&[52, 53]).unwrap();
    let bomb2 = detect(&[c(12, 0), c(12, 1), c(12, 2), c(12, 3)]).unwrap();
    assert!(beats(&rocket, Some(&bomb2)));
    assert!(!beats(&bomb2, Some(&rocket)));
    assert_eq!(Rank(12).label(), "2");
}

#[test]
fn ai_lead_prefers_long_combo_over_single() {
    // 资深玩家抓到的牌感缺陷：AI 拿顺子却出单张。
    let straight_hand: Vec<u8> = (0..7u8)
        .map(|r| c(r, 0))
        .chain(std::iter::once(c(12, 0)))
        .collect();
    let pick = ai_choose(&straight_hand, None, 1, false, Some(0));
    assert_eq!(pick.map(|p| p.len()), Some(7), "应出 7 张顺子");
    let pair_run = vec![
        c(3, 0),
        c(3, 1),
        c(4, 0),
        c(4, 1),
        c(5, 0),
        c(5, 1),
        c(12, 0),
    ];
    let pick2 = ai_choose(&pair_run, None, 1, false, Some(0));
    assert_eq!(pick2.map(|p| p.len()), Some(6), "应出连对 6 张");
}
