//! 斗地主 AI 同型压制单测（从 doudizhu_ai_combo.rs 拆出：门禁）。

use super::doudizhu::{detect, ComboKind, Rank};
use super::doudizhu_ai_combo::counter_candidates;

fn c(rank: u8, suit: usize) -> u8 {
    if rank >= 13 {
        52 + (rank - 13)
    } else {
        rank * 4 + suit as u8
    }
}

#[test]
fn counters_straight_with_higher_run() {
    let hand: Vec<u8> = (3..10u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect();
    // prev = 34567
    let run: Vec<u8> = (0..5u8).map(|r| c(r, r as usize % 4)).collect();
    let prev = detect(&run).unwrap();
    let cands = counter_candidates(&hand, &prev, false);
    assert!(
        cands.iter().any(|c| c.kind == ComboKind::Straight),
        "应能出更大顺子压制（此前 match 落空必过牌）"
    );
}

#[test]
fn counters_pair_straight() {
    let hand: Vec<u8> = (2..8u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect();
    let prev_run: Vec<u8> = (0..3u8).flat_map(|r| [c(r, 0), c(r, 1)]).collect();
    let prev = detect(&prev_run).unwrap();
    let cands = counter_candidates(&hand, &prev, false);
    assert!(cands.iter().any(|c| c.kind == ComboKind::PairStraight));
}

#[test]
fn counters_triple_single_with_wing() {
    // 手牌：555 + 散 3 → 三带一压 444 带 2
    let hand = vec![c(5, 0), c(5, 1), c(5, 2), c(0, 3)];
    let prev_cards = vec![c(1, 0), c(1, 1), c(1, 2), c(2, 3)];
    let prev = detect(&prev_cards).unwrap();
    let cands = counter_candidates(&hand, &prev, false);
    assert!(
        cands.iter().any(|c| c.kind == ComboKind::TripleSingle),
        "应能凑三带一压制（此前只找裸三张被 beats 拒光）"
    );
}

#[test]
fn counters_plane_with_wings() {
    // 手牌：666 777 + 散 3、4 → 飞机带单压 333444 带 5、6
    let hand = vec![
        c(6, 0),
        c(6, 1),
        c(6, 2),
        c(7, 0),
        c(7, 1),
        c(7, 2),
        c(0, 3),
        c(1, 3),
    ];
    let mut prev_cards: Vec<u8> = Vec::new();
    prev_cards.extend((0..3).map(|s| c(0, s)));
    prev_cards.extend((0..3).map(|s| c(1, s)));
    prev_cards.push(c(2, 0));
    prev_cards.push(c(3, 0));
    let prev = detect(&prev_cards).unwrap();
    let cands = counter_candidates(&hand, &prev, false);
    assert!(
        cands.iter().any(|c| matches!(
            c.kind,
            ComboKind::PlaneSingle | ComboKind::Plane
        )),
        "应能出飞机压制（此前无候选）"
    );
}

#[test]
fn triple_wing_excludes_fourth_body_card() {
    // 5555 + 3：机身 take 3 张 5，翅膀不得再取第 4 张 5
    // （旧 bug：按实体排除漏掉第 4 张，凑出 6 张非法组合丢候选）
    let hand = vec![c(5, 0), c(5, 1), c(5, 2), c(5, 3), c(0, 0)];
    let prev_cards = vec![c(1, 0), c(1, 1), c(1, 2), c(2, 3)];
    let prev = detect(&prev_cards).unwrap();
    let cands = counter_candidates(&hand, &prev, false);
    let hit = cands
        .iter()
        .find(|c| c.kind == ComboKind::TripleSingle)
        .expect("应能凑三带一");
    assert_eq!(hit.len, 4, "三带一恰 4 张");
    let fives = hit.cards.iter().filter(|&&x| Rank::of(x).0 == 5).count();
    assert_eq!(fives, 3, "机身只能 3 张 5，第 4 张不得混入");
}

#[test]
fn four_with_two_not_needed_as_counter() {
    // 四带二压制不在候选面（罕见型），确认不 panic 且不产非法组合
    let hand = vec![c(8, 0), c(8, 1), c(8, 2), c(8, 3), c(0, 0), c(1, 1)];
    let prev_cards = vec![c(3, 0), c(3, 1), c(3, 2), c(3, 3), c(0, 1), c(1, 2)];
    let prev = detect(&prev_cards).unwrap();
    assert_eq!(prev.kind, ComboKind::FourTwoSingle);
    let cands = counter_candidates(&hand, &prev, false);
    assert!(cands.iter().all(|c| detect(&c.cards).is_some()));
}
