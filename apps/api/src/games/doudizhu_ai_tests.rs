//! 斗地主 AI 单测（从 doudizhu_ai.rs 拆出：主文件贴 300 行门禁）。

use super::doudizhu::{detect, ComboKind, Rank};
use super::doudizhu_ai::{ai_bid, ai_choose, counter_candidates, finisher};

fn c(rank: u8, suit: usize) -> u8 {
    if rank >= 13 {
        52 + (rank - 13)
    } else {
        rank * 4 + suit as u8
    }
}

#[test]
fn ai_bid_calls_three_with_strong_hand() {
    // 双王+双2 → suggested 3，AI 必须敢叫 3，
    // 不能像旧版封顶 2 让「玩家叫 3 必得地主」。
    assert_eq!(ai_bid(3, 0), 3);
    assert_eq!(ai_bid(3, 2), 3);
}

#[test]
fn ai_bid_follows_only_when_affordable() {
    assert_eq!(ai_bid(2, 0), 2, "中等牌力领先者叫满");
    assert_eq!(ai_bid(2, 1), 2, "跟叫到自身上限");
    assert_eq!(ai_bid(2, 2), 0, "跟不起就不叫");
    assert_eq!(ai_bid(0, 0), 0, "弱牌不叫");
}

#[test]
fn counter_finds_smallest_beating_single() {
    let hand = vec![c(0, 0), c(2, 1), c(5, 2), c(9, 3)];
    let prev = detect(&[c(1, 0)]).unwrap(); // 单 4
    let cands = counter_candidates(&hand, &prev, true);
    assert!(!cands.is_empty());
    assert_eq!(cands[0].major, Rank(2), "最小压过的单 5");
}

#[test]
fn counter_empty_when_nothing_beats() {
    let hand = vec![c(0, 0), c(1, 1)];
    let prev = detect(&[c(11, 0)]).unwrap(); // 单 A
    let cands = counter_candidates(&hand, &prev, false);
    assert!(cands.is_empty(), "无 2 无王压不动");
}

#[test]
fn bomb_only_when_allowed() {
    let hand = vec![c(3, 0), c(3, 1), c(3, 2), c(3, 3), c(0, 0)];
    let prev = detect(&[c(1, 0)]).unwrap();
    let with = counter_candidates(&hand, &prev, true);
    assert!(with.iter().any(|c| c.kind == ComboKind::Bomb));
    let without = counter_candidates(&hand, &prev, false);
    assert!(!without.iter().any(|c| c.kind == ComboKind::Bomb));
}

#[test]
fn ai_lead_prefers_small_single() {
    let hand = vec![c(4, 0), c(4, 1), c(8, 0), c(0, 2)];
    let pick = ai_choose(&hand, None, 1, false, None).unwrap();
    assert_eq!(pick.len(), 1);
    assert_eq!(Rank::of(pick[0]), Rank(0), "领出最小单 3");
}

#[test]
fn ai_counters_with_smallest_and_hoards_bombs() {
    // 手里有单 5 和一组炸弹(3)
    let hand = vec![c(2, 0), c(0, 1), c(0, 2), c(0, 3), c(0, 0)];
    let prev = detect(&[c(1, 0)]).unwrap(); // 对家单 4
    let pick = ai_choose(&hand, Some((&prev, 2)), 1, false, None).unwrap();
    assert_eq!(pick.len(), 1);
    assert_eq!(Rank::of(pick[0]), Rank(2), "出单 5 压，不出炸弹");
}

#[test]
fn ai_releases_bomb_in_endgame() {
    let hand = vec![c(3, 0), c(3, 1), c(3, 2), c(3, 3)];
    let prev = detect(&[c(11, 0)]).unwrap(); // 单 A
    let pick = ai_choose(&hand, Some((&prev, 2)), 1, false, None);
    assert!(pick.is_some(), "残局 4 张炸弹该出手");
    assert_eq!(pick.unwrap().len(), 4);
}

#[test]
fn finisher_spots_one_shot() {
    assert!(finisher(&[c(0, 0)]).is_some());
    // 34567 顺子一手
    let run: Vec<u8> = (0..5u8).map(|r| c(r, (r % 4) as usize)).collect();
    assert!(finisher(&run).is_some());
    // 杂牌两手出不完
    let messy = vec![c(0, 0), c(2, 1), c(5, 2), c(9, 0)];
    assert!(finisher(&messy).is_none());
}

#[test]
fn teammate_low_holds_fire() {
    let hand = vec![c(5, 0)];
    let prev = detect(&[c(1, 0)]).unwrap();
    // 队友出的且队友快走完 → 不抢
    assert!(ai_choose(&hand, Some((&prev, 2)), 1, true, Some(0)).is_none());
    // 对家（地主）出的 → 照压
    assert!(ai_choose(&hand, Some((&prev, 0)), 1, true, Some(0)).is_some());
}

#[test]
fn teammate_faction_follows_landlord_not_seat() {
    // 旧实现按座位号硬判 1↔2 互为队友：下面两种局面会被误判成队友而放水。
    // 局面 A：AI(1) 是地主，AI(2) 是农民 → seat2 必须压地主，不许放水。
    let hand = vec![c(5, 0)]; // 单 6
    let prev = detect(&[c(1, 0)]).unwrap(); // 单 4
    assert!(
        ai_choose(&hand, Some((&prev, 1)), 2, true, Some(1)).is_some(),
        "AI 地主出牌时农民不得当队友放水"
    );
    // 局面 B：AI(2) 是地主，AI(1) 是农民 → seat1 必须压地主。
    assert!(
        ai_choose(&hand, Some((&prev, 2)), 1, true, Some(2)).is_some(),
        "AI 地主出牌时农民不得当队友放水"
    );
    // 局面 C：玩家(0) 是地主 → seat1/2 才是真队友，手牌少时不抢。
    assert!(
        ai_choose(&hand, Some((&prev, 1)), 2, true, Some(0)).is_none(),
        "玩家地主时两 AI 才是真队友"
    );
    // 局面 D：地主未定 → 不放水（保守出牌）
    assert!(ai_choose(&hand, Some((&prev, 1)), 2, true, None).is_some());
}
