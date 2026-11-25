//! 斗地主 AI 同型压制候选（从 doudizhu_ai.rs 拆出：主文件贴 300 行
//! 门禁）。单/对/三带/顺子/连对/飞机全型压制——此前 AI 对顺子/连对/
//! 飞机/三带一无任何候选（match 落空必过牌），牌力明显弱于真人。

use super::doudizhu::{beats, detect, Combo, ComboKind, Rank};

fn counts_of(hand: &[u8]) -> [u8; 15] {
    let mut counts = [0u8; 15];
    for &c in hand {
        counts[Rank::of(c).0 as usize] += 1;
    }
    counts
}

/// 生成「压制 prev」的全部同型候选（含炸弹/王炸兜底），候选按
/// major 升序返回。`allow_bomb=false` 时不产炸弹（惜弹场景）。
pub fn counter_candidates(
    hand: &[u8],
    prev: &Combo,
    allow_bomb: bool,
) -> Vec<Combo> {
    let mut out = Vec::new();
    let counts = counts_of(hand);
    let mut same_kind = |need: usize, filter: &dyn Fn(usize) -> bool| {
        for r in 0..15u8 {
            let n = usize::from(counts[r as usize]);
            if filter(r as usize) && n >= need {
                let cards: Vec<u8> = hand
                    .iter()
                    .copied()
                    .filter(|&c| Rank::of(c).0 == r)
                    .take(need)
                    .collect();
                if let Some(c) = detect(&cards) {
                    if beats(&c, Some(prev)) {
                        out.push(c);
                    }
                }
            }
        }
    };
    match prev.kind {
        ComboKind::Single => {
            same_kind(1, &|r| counts[r] >= 1 && Rank(r as u8) > prev.major)
        }
        ComboKind::Pair => {
            same_kind(2, &|r| counts[r] >= 2 && Rank(r as u8) > prev.major)
        }
        ComboKind::Triple => {
            same_kind(3, &|r| counts[r] >= 3 && Rank(r as u8) > prev.major)
        }
        // 三带一/带对：找更大的裸三张，再从余牌挑最弱的翅膀凑形。
        ComboKind::TripleSingle | ComboKind::TriplePair => {
            let wing = if prev.kind == ComboKind::TripleSingle {
                1
            } else {
                2
            };
            for r in 0..13u8 {
                if counts[r as usize] < 3 || Rank(r) <= prev.major {
                    continue;
                }
                let body: Vec<u8> = hand
                    .iter()
                    .copied()
                    .filter(|&c| Rank::of(c).0 == r)
                    .take(3)
                    .collect();
                // 翅膀从非机身 rank 取（按 rank 排除而非实体牌排除——
                // 手里第 4 张机身 rank 不在 body 实体里，旧写法会把它
                // 当翅膀凑出非法 6 张组合）。
                let wing_cards: Vec<u8> = hand
                    .iter()
                    .copied()
                    .filter(|&c| Rank::of(c).0 != r)
                    .take(wing)
                    .collect();
                if wing_cards.len() == wing {
                    let mut full = body.clone();
                    full.extend(wing_cards);
                    if let Some(c) = detect(&full) {
                        if beats(&c, Some(prev)) {
                            out.push(c);
                        }
                    }
                }
            }
        }
        // 顺子 / 连对：滑窗找 ≥ 上的同长同型
        ComboKind::Straight => {
            let len = prev.len;
            for hi in (prev.major.0 + 1)..12u8 {
                if hi + 1 < len as u8 {
                    continue;
                }
                let lo = hi - (len as u8 - 1);
                let ranks_ok =
                    (lo..=hi).all(|r| counts[r as usize] >= 1 && r < 12);
                if !ranks_ok {
                    continue;
                }
                let mut pick: Vec<u8> = Vec::new();
                for r in lo..=hi {
                    if let Some(found) =
                        hand.iter().find(|&&c| Rank::of(c).0 == r)
                    {
                        pick.push(*found);
                    }
                }
                if let Some(c) = detect(&pick) {
                    if beats(&c, Some(prev)) {
                        out.push(c);
                    }
                }
            }
        }
        ComboKind::PairStraight => {
            let len = prev.len / 2;
            for hi in (prev.major.0 + 1)..12u8 {
                if hi + 1 < len as u8 {
                    continue;
                }
                let lo = hi - (len as u8 - 1);
                let ranks_ok =
                    (lo..=hi).all(|r| counts[r as usize] >= 2 && r < 12);
                if !ranks_ok {
                    continue;
                }
                let mut pick: Vec<u8> = Vec::new();
                for r in lo..=hi {
                    let got: Vec<u8> = hand
                        .iter()
                        .copied()
                        .filter(|&c| Rank::of(c).0 == r)
                        .take(2)
                        .collect();
                    pick.extend(got);
                }
                if let Some(c) = detect(&pick) {
                    if beats(&c, Some(prev)) {
                        out.push(c);
                    }
                }
            }
        }
        // 飞机（含带翅膀）：找连续 ≥2 组更大三张，翅膀从余牌补齐
        ComboKind::Plane | ComboKind::PlaneSingle | ComboKind::PlanePair => {
            let groups = prev.len / 3
                - if prev.kind == ComboKind::Plane { 0 } else { 1 };
            let groups = groups.max(2);
            let wing_each = match prev.kind {
                ComboKind::PlaneSingle => 1,
                ComboKind::PlanePair => 2,
                _ => 0,
            };
            let mut triples_avail: Vec<u8> = (0..12u8)
                .filter(|&r| counts[r as usize] >= 3 && Rank(r) > prev.major)
                .collect();
            // 找连续 groups 段（从大到小取最小编号以省大牌）
            while triples_avail.len() >= groups {
                let win = &triples_avail[..groups];
                let consecutive = win.windows(2).all(|w| w[1] == w[0] + 1);
                if !consecutive || win[groups - 1] >= 12 {
                    triples_avail.remove(0);
                    continue;
                }
                let mut pick: Vec<u8> = Vec::new();
                for &r in win {
                    let got: Vec<u8> = hand
                        .iter()
                        .copied()
                        .filter(|&c| Rank::of(c).0 == r)
                        .take(3)
                        .collect();
                    pick.extend(got);
                }
                if wing_each > 0 {
                    // 翅膀同样按 rank 排除机身（第 4 张机身 rank 不是翅膀）
                    let wings: Vec<u8> = hand
                        .iter()
                        .copied()
                        .filter(|&c| !win.iter().any(|&r| Rank::of(c).0 == r))
                        .take(groups * wing_each)
                        .collect();
                    if wings.len() != groups * wing_each {
                        triples_avail.remove(0);
                        continue;
                    }
                    pick.extend(wings);
                }
                if let Some(c) = detect(&pick) {
                    if beats(&c, Some(prev)) {
                        out.push(c);
                    }
                }
                triples_avail.remove(0);
            }
        }
        _ => {}
    }
    if allow_bomb {
        // 炸弹压同型与非炸弹；王炸最后
        for r in (0..13u8).rev() {
            let beats_prev =
                prev.kind != ComboKind::Bomb || Rank(r) > prev.major;
            if counts[r as usize] == 4
                && beats_prev
                && prev.kind != ComboKind::Rocket
            {
                let cards: Vec<u8> = hand
                    .iter()
                    .copied()
                    .filter(|&c| Rank::of(c).0 == r)
                    .collect();
                if let Some(c) = detect(&cards) {
                    out.push(c);
                }
            }
        }
        if counts[13] > 0 && counts[14] > 0 && prev.kind != ComboKind::Rocket {
            if let Some(c) = detect(&[52, 53]) {
                out.push(c);
            }
        }
    }
    out.retain(|c| beats(c, Some(prev)));
    out
}
