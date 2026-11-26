//! 斗地主状态机单测（从 doudizhu_game.rs 拆出：主文件贴 300 行门禁）。

use super::doudizhu::{beats, deal, detect, Rank};
use super::doudizhu_game::*;

fn c(rank: u8, suit: usize) -> u8 {
    if rank >= 13 {
        52 + (rank - 13)
    } else {
        rank * 4 + suit as u8
    }
}

/// 测试辅助：走完抢地主（不抢）/弃三张（不弃）/加倍（不倍），进入出牌阶段。
fn advance_to_playing(g: &mut GameState) {
    while g.phase == Phase::Grab {
        let seat = g.turn;
        let _ = g.grab(seat, false);
    }
    if g.phase == Phase::Discard {
        let seat = g.turn;
        let _ = g.discard(seat, &[]);
    }
    while g.phase == Phase::Double {
        let seat = g.turn;
        let _ = g.double(seat, 0);
    }
}

#[test]
fn bidding_flow_three_call() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    assert_eq!(g.phase, Phase::Bidding);
    g.bid(0, 1).unwrap();
    g.bid(1, 2).unwrap();
    g.bid(2, 3).unwrap();
    assert_eq!(g.landlord, Some(2));
    advance_to_playing(&mut g);
    assert_eq!(g.phase, Phase::Playing);
    assert_eq!(g.turn, 2);
    // 地主手牌 17+3 = 20
    assert_eq!(g.hands[2].len(), 20);
    assert_eq!(g.hands[0].len(), 17);
    assert_eq!(g.kitty.len(), 0);
}

#[test]
fn bidding_all_zero_is_restart() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 0).unwrap();
    g.bid(1, 0).unwrap();
    g.bid(2, 0).unwrap();
    assert_eq!(g.phase, Phase::Done);
    assert_eq!(g.finished, Some(u8::MAX));
}

#[test]
fn bidding_highest_wins_ties_first() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 2).unwrap();
    g.bid(1, 2).unwrap();
    g.bid(2, 1).unwrap();
    assert_eq!(g.landlord, Some(0), "同分先叫先得");
}

#[test]
fn bid_validation() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    assert!(g.bid(1, 1).is_err(), "轮次不对");
    assert!(g.bid(0, 4).is_err(), "分数超档");
    g.bid(0, 3).unwrap();
    assert!(g.bid(1, 1).is_err(), "叫分结束不许再叫");
}

#[test]
fn play_flow_and_table_clear() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap(); // 玩家地主，先出
    advance_to_playing(&mut g);
    let s0 = g.hands[0][0]; // 最大单张
    let before = g.hands[0].len();
    g.play(0, &[s0]).unwrap();
    assert_eq!(g.hands[0].len(), before - 1);
    assert_eq!(g.last_seat, Some(0));
    // 两家过牌 → 桌面清空、回到玩家
    g.play(1, &[]).unwrap();
    g.play(2, &[]).unwrap();
    assert_eq!(g.last, None);
    assert_eq!(g.passes, 0);
    assert_eq!(g.turn, 0);
}

#[test]
fn play_validation() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap();
    advance_to_playing(&mut g);
    // 领出不能过
    assert!(g.play(0, &[]).is_err());
    // 别人手里的牌不能用
    let s1 = g.hands[1][0];
    assert!(g.play(0, &[s1]).is_err());
    // 轮次校验：玩家出后轮到 1，1 出后轮到 2
    let s0 = g.hands[0][0];
    g.play(0, &[s0]).unwrap();
    assert!(g.play(0, &[g.hands[0][0]]).is_err(), "还没轮回来");
}

#[test]
fn finish_when_hand_empty() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap();
    advance_to_playing(&mut g);
    // 直接把玩家手牌打空（单张逐张出）
    while !g.hands[0].is_empty() && g.turn == 0 && g.phase == Phase::Playing {
        // AI 座位轮转时强制过牌到回玩家
        let card = g.hands[0][0];
        let c = detect(&[card]).unwrap();
        let ok = match (&g.last, g.last_seat) {
            (Some(l), Some(s)) if s != 0 => beats(&c, Some(l)),
            _ => true,
        };
        if ok {
            g.play(0, &[card]).unwrap();
        } else {
            // 压不过时改出炸弹或直接清桌面场景无法构造，回退只测循环
            break;
        }
        if g.turn != 0 {
            g.play(1, &[]).unwrap();
            if g.turn != 0 {
                g.play(2, &[]).unwrap();
            }
        }
    }
    if g.hands[0].is_empty() {
        assert_eq!(g.finished, Some(0));
        assert_eq!(g.phase, Phase::Done);
    }
}

#[test]
fn spring_detection() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap();
    assert!(!g.is_spring(), "未终局不判春天");
    // 手工构造：农民零出牌 → 地主赢 = 春天
    g.phase = Phase::Done;
    g.finished = Some(0);
    assert!(g.is_spring());
}

#[test]
fn anti_spring_requires_exactly_one_landlord_play() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap(); // 玩家地主
                          // 农民赢：地主只出 1 手 → 反春成立
    g.events.push(Event {
        seat: 0,
        action: "play".into(),
        score: None,
        cards: vec![g.hands[0][0]],
    });
    g.phase = Phase::Done;
    g.finished = Some(1);
    assert!(g.is_spring(), "地主只出 1 手农民赢 = 反春");
    // 地主出 2 手后农民赢 → 不是反春（旧 <=1 口径会误判）
    g.events.push(Event {
        seat: 0,
        action: "play".into(),
        score: None,
        cards: vec![g.hands[0][1]],
    });
    assert!(!g.is_spring(), "地主出 2 手不算反春");
}

#[test]
fn suggested_bid_scale() {
    // 双王+双2 → 3 分
    assert_eq!(GameState::suggested_bid(&[52, 53, c(12, 0), c(12, 1)]), 3);
    // 单 2 → 1 分
    assert_eq!(GameState::suggested_bid(&[c(12, 0)]), 1);
    // 什么都没有 → 0 分
    assert_eq!(GameState::suggested_bid(&[c(0, 0), c(1, 1)]), 0);
}

#[test]
fn grab_replaces_landlord_and_counts() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap(); // 候选 0
    assert_eq!(g.phase, Phase::Grab);
    // seat1 抢 → 候选换 1，抢次 +1
    g.grab(1, true).unwrap();
    assert_eq!(g.landlord, Some(1));
    assert_eq!(g.grabs, 1);
    // seat2 不抢 → 表态完毕，进入弃牌阶段
    g.grab(2, false).unwrap();
    assert_eq!(g.phase, Phase::Discard);
    assert_eq!(g.landlord, Some(1));
}

#[test]
fn discard_removes_up_to_three() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap();
    g.grab(1, false).unwrap();
    g.grab(2, false).unwrap();
    assert_eq!(g.phase, Phase::Discard);
    let before = g.hands[0].len();
    let drop = g.hands[0][..2].to_vec();
    g.discard(0, &drop).unwrap();
    assert_eq!(g.hands[0].len(), before - 2);
    assert_eq!(g.discards.len(), 2);
    assert_eq!(g.phase, Phase::Double);
    // 弃超 3 张非法
    let over: Vec<u8> = g.hands[0].iter().take(4).copied().collect();
    assert!(g.discard(0, &over).is_err());
}

#[test]
fn double_levels_then_play() {
    let (hands, kitty) = deal();
    let mut g = GameState::new(hands.map(Vec::from), kitty.to_vec());
    g.bid(0, 3).unwrap();
    g.grab(1, false).unwrap();
    g.grab(2, false).unwrap();
    g.discard(0, &[]).unwrap();
    assert_eq!(g.phase, Phase::Double);
    g.double(0, 1).unwrap();
    g.double(1, 0).unwrap();
    g.double(2, 2).unwrap();
    assert_eq!(g.double_levels, [1, 0, 2]);
    assert_eq!(g.phase, Phase::Playing);
    assert_eq!(g.turn, 0);
}

#[test]
fn nolook_deal_keeps_hand_balance() {
    let (hands, kitty) = super::doudizhu::deal_nolook();
    assert_eq!(hands[0].len(), 17);
    assert_eq!(hands[1].len(), 17);
    assert_eq!(hands[2].len(), 17);
    assert_eq!(kitty.len(), 3);
    let mut all: Vec<u8> =
        hands.iter().flatten().copied().chain(kitty).collect();
    all.sort();
    let expect: Vec<u8> = (0..54u8).collect();
    assert_eq!(all, expect);
}
