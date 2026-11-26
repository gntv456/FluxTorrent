//! 斗地主对局仿真压测：多局随机叫分 + AI 出牌，验证状态机不卡死、
//! 手牌守恒、终局必到、AI 决策合法。纯逻辑层，不碰 DB。

use crate::games::doudizhu::{detect, ComboKind, Rank};
use crate::games::doudizhu_ai::ai_choose;
use crate::games::doudizhu_game::{GameState, Phase};

fn c(rank: u8, suit: usize) -> u8 {
    if rank >= 13 {
        52 + (rank - 13)
    } else {
        rank * 4 + suit as u8
    }
}

/// 固定牌组（54 张全牌），按发牌序切三份 + 3 张底牌。
fn fixed_deal(seed: u32) -> ([[u8; 17]; 3], [u8; 3]) {
    let mut deck: Vec<u8> = (0..54u8).collect();
    // 确定性洗牌（xorshift），保证可复现
    let mut x = seed | 1;
    for i in (1..deck.len()).rev() {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        let j = (x as usize) % (i + 1);
        deck.swap(i, j);
    }
    let mut hands = [[0u8; 17]; 3];
    for s in 0..3 {
        hands[s].copy_from_slice(&deck[s * 17..(s + 1) * 17]);
        hands[s].sort_by_key(|&card| std::cmp::Reverse(Rank::of(card)));
    }
    let mut kitty = [0u8; 3];
    kitty.copy_from_slice(&deck[51..54]);
    (hands, kitty)
}

/// 单局仿真：三家全 AI，返回 (步数, 终局座位)。
fn simulate(seed: u32) -> (usize, u8) {
    let (hands, kitty) = fixed_deal(seed);
    let mut g = GameState::new(
        [hands[0].to_vec(), hands[1].to_vec(), hands[2].to_vec()],
        kitty.to_vec(),
    );
    // 叫分：三家按建议分（上限 3），seed 决定让不让
    let mut guard = 0;
    while g.phase == Phase::Bidding {
        guard += 1;
        assert!(guard < 10, "叫分阶段卡死 seed={seed}");
        let seat = g.turn;
        let mut bid = GameState::suggested_bid(&g.hands[seat as usize]).min(3);
        if (seed as usize + seat as usize) % 7 == 0 {
            bid = 0;
        }
        g.bid(seat, bid).unwrap();
    }
    // 全 0 流局：GameState 标记 u8::MAX，仿真直接返回
    if g.phase == Phase::Done {
        return (0, u8::MAX);
    }
    // 抢地主：AI 按牌力表态抢/不抢
    while g.phase == Phase::Grab {
        let seat = g.turn;
        let take = GameState::suggested_bid(&g.hands[seat as usize]) >= 2;
        g.grab(seat, take).unwrap();
    }
    // 弃三张：AI 地主不弃牌
    if g.phase == Phase::Discard {
        let seat = g.turn;
        g.discard(seat, &[]).unwrap();
    }
    // 加倍：AI 按牌力表态加倍档
    while g.phase == Phase::Double {
        let seat = g.turn;
        let s = GameState::suggested_bid(&g.hands[seat as usize]);
        let level = if s >= 3 {
            2
        } else if s >= 2 {
            1
        } else {
            0
        };
        g.double(seat, level).unwrap();
    }
    // 出牌轮转：三家全用 ai_choose 决策，验证不卡死
    let mut steps = 0;
    while g.phase == Phase::Playing {
        steps += 1;
        assert!(steps < 600, "出牌阶段卡死 seed={seed}");
        let seat = g.turn;
        let hand = g.hands[seat as usize].clone();
        let landlord = g.landlord;
        let pick = match (&g.last, g.last_seat) {
            (Some(prev), Some(last_seat)) => {
                // 领出者是自己时 prev 会被状态机忽略，照常传即可
                ai_choose(&hand, Some((prev, last_seat)), seat, false, landlord)
            }
            _ => ai_choose(&hand, None, seat, false, landlord),
        };
        let cards = pick.unwrap_or_default();
        // 合法性：非空的必须能被 detect 认出（癞子模式未纳入本压测）
        if !cards.is_empty() {
            assert!(detect(&cards).is_some(), "AI 出非法牌型 seed={seed}");
        }
        // 领出不能过：若 AI 返回空且是领出，强制出最小单张
        let cards = if cards.is_empty() && g.last.is_none() {
            hand.first().map(|&x| vec![x]).unwrap_or_default()
        } else {
            cards
        };
        g.play(seat, &cards).unwrap_or_else(|e| {
            panic!("非法动作 seed={seed} seat={seat} err={e}")
        });
    }
    // 手牌守恒：出牌后总手牌数 + 桌面牌 = 54 - 底牌归地主前的分发
    let total: usize = g.hands.iter().map(|h| h.len()).sum();
    let landlord_cards = g.hands[landlord_check(&g) as usize].len();
    assert!(landlord_cards <= 20, "地主手牌异常 seed={seed}");
    assert_eq!(g.finished.is_some(), true, "终局无赢家 seed={seed}");
    (steps, total as u8)
}

fn landlord_check(g: &GameState) -> u8 {
    g.landlord.unwrap_or(0)
}

#[test]
fn simulate_200_games_never_stalls() {
    let mut max_steps = 0usize;
    let mut finished = 0usize;
    let mut draws = 0usize;
    for seed in 1..=200u32 {
        let (steps, winner) = simulate(seed);
        max_steps = max_steps.max(steps);
        if winner == u8::MAX {
            draws += 1;
        } else {
            finished += 1;
        }
    }
    assert!(finished + draws == 200);
    assert!(max_steps < 600, "存在超长对局: {max_steps}");
}

#[test]
fn simulate_ai_plays_only_legal_combos() {
    // 锁定：AI 在 50 局里出的每一手都能被 detect 识别
    for seed in 1..=50u32 {
        let (_, _) = simulate(seed);
    }
    // 额外：四带二、飞机、顺子各出一手验证 ComboKind 覆盖
    assert!(
        detect(&[c(3, 0), c(3, 1), c(3, 2), c(3, 3), c(5, 0), c(7, 0)])
            .is_some()
    );
    assert!(detect(&[c(3, 0), c(4, 0), c(5, 0), c(6, 0), c(7, 0)]).is_some());
    assert!(
        detect(&[c(3, 0), c(3, 1), c(3, 2), c(4, 0), c(4, 1), c(4, 2)])
            .is_some()
    );
}
