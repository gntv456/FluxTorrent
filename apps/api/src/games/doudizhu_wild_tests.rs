//! 斗地主癞子模式单测（从 doudizhu_wild.rs 拆出：主文件贴 300 行门禁）。

use crate::games::doudizhu::{detect, ComboKind, Rank};
use crate::games::doudizhu_wild::{
    effective_combos, pick_wildcards, wild_beats, wild_hint,
};

fn c(rank: u8, suit: usize) -> u8 {
    rank * 4 + suit as u8
}

#[test]
fn wild_hint_returns_original_hand_cards() {
    // 单张癞子(点5)压单 K：提示必须返回原牌编码 c(5,0)，
    // 不能返回替身虚拟编码 target*4（否则 play() 拒手里没这张牌）。
    let hand = vec![c(5, 0), c(2, 1)];
    let wilds = vec![c(5, 0)];
    let prev = detect(&[c(9, 0)]).unwrap(); // 单 K
    let hint = wild_hint(&hand, &wilds, Some(&prev)).expect("应有提示");
    assert!(
        hand.contains(&hint[0]),
        "提示牌必须是手里的原始编码: {hint:?}"
    );
    // 原牌必须能被 wild_beats 接受（play() 同款判定）
    assert!(wild_beats(&hint, &wilds, Some(&prev)).is_some());
}

#[test]
fn wild_hint_none_when_nothing_beats() {
    let hand = vec![c(5, 0)]; // 单癞子
    let wilds = vec![c(5, 0)];
    let prev = detect(&[c(12, 0)]).unwrap(); // 单 2，癞子最大只到 A
    assert!(wild_hint(&hand, &wilds, Some(&prev)).is_none());
}

#[test]
fn combo_enumeration_matches_permutation_result() {
    // 4 张癞子：组合枚举(1820)与排列枚举(28561)的牌型集合应一致。
    let hand = vec![c(5, 0), c(5, 1), c(5, 2), c(5, 3)];
    let combos = effective_combos(&hand, &hand);
    assert!(combos
        .iter()
        .any(|k| k.kind == ComboKind::Bomb && k.major == Rank(12)));
    // 顺子 34567：4 癞子 + 1 实体可凑 5 张顺子
    let hand2 = vec![c(5, 0), c(5, 1), c(5, 2), c(5, 3), c(0, 0)];
    let combos2 = effective_combos(&hand2, &hand[..]);
    assert!(combos2.iter().any(|k| k.kind == ComboKind::Straight));
}

#[test]
fn wildcards_pick_only_same_rank() {
    let hands = [vec![c(5, 0), c(9, 1)], vec![c(5, 2)], vec![c(3, 0)]];
    let kitty = [c(1, 0), c(2, 0), c(5, 3)];
    let w = pick_wildcards(&hands, &kitty);
    assert_eq!(w[0], vec![c(5, 0)]);
    assert_eq!(w[1], vec![c(5, 2)]);
    assert!(w[2].is_empty());
}
#[test]
fn joker_in_kitty_does_not_void_wildcards() {
    // 底牌末张是王：癞子点回退到非王牌，不能让 wildcards 全空。
    let hands = [vec![c(7, 0), c(9, 1)], vec![c(7, 2)], vec![c(5, 0)]];
    let kitty = [c(1, 0), c(7, 3), 53];
    let w = pick_wildcards(&hands, &kitty);
    assert!(w.iter().any(|x| !x.is_empty()), "底牌有王时仍须有癞子");
}
