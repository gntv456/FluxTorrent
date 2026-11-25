//! 斗地主 AI：叫分建议在 [`super::doudizhu_game::GameState::suggested_bid`]，
//! 本文件管出牌决策——候选生成 + 简单启发式评估。刻意不搜索对局树：
//! AI 是「氛围组」，牌力与玩家平均持平即可，经济模型不依赖 AI 强弱。
//!
//! 策略分层：
//! 1. 压不起/不需要压 → 从手牌拆出「最不伤结构」的领出；
//! 2. 需要压 → 在同型候选里取**最小的能压过的**（惜弹：炸弹只留给残局）；
//! 3. 残局（手牌 ≤2 组可一次出完）优先寻找一次清仓的连招。
//!
//! 农民配合：按 landlord 判真实阵营，队友（另一农民）出的牌且手牌
//! ≤4 时不抢；AI 当地主时不得把地主当队友而放水。

use super::doudizhu::{beats, detect, Combo, ComboKind, Rank};

/// 手牌 rank 计数表。
fn counts_of(hand: &[u8]) -> [u8; 15] {
    let mut counts = [0u8; 15];
    for &c in hand {
        counts[Rank::of(c).0 as usize] += 1;
    }
    counts
}

/// 从手牌按编码取牌（不存在返回 None）。
fn take(hand: &[u8], cards: &[u8]) -> Option<Vec<u8>> {
    let mut rest = hand.to_vec();
    for &c in cards {
        let pos = rest.iter().position(|&k| k == c)?;
        rest.remove(pos);
    }
    Some(rest)
}

/// 留牌价值：越小越想先出。单张/散牌低值，强结构（炸弹/王）高分。
fn hand_value(hand: &[u8]) -> i64 {
    let counts = counts_of(hand);
    let mut v = 0;
    for (r, &cnt) in counts.iter().enumerate() {
        v += match cnt {
            0 => 0,
            1 => r as i64,           // 单张按大小
            2 => (r as i64) * 2,     // 对子
            3 => (r as i64) * 3 + 2, // 三张略抬
            _ => 40,                 // 炸弹高价值
        };
    }
    if counts[14] > 0 {
        v += 20;
    } // 大王加成
    v
}

/// AI 叫分策略：牌力够直接叫 3 抢地主；否则比当前最高多 1 分，
/// 但不超过牌力上限。此前 AI 永远封顶 2 分，玩家叫 3 必得地主。
pub fn ai_bid(suggested: i64, highest: i64) -> i64 {
    if suggested >= 3 {
        return 3; // 双王+2 等强牌，直接 3 分
    }
    if highest >= suggested {
        return 0; // 跟不起就不叫
    }
    if highest == 0 {
        return suggested; // 首个叫分：直接叫满自身牌力
    }
    (highest + 1).min(suggested)
}

/// 生成「压制 prev」的全部同型候选（含炸弹/王炸兜底）。
/// 实现下沉 `doudizhu_ai_combo::counter_candidates`（主文件贴 300 门禁）。
pub fn counter_candidates(
    hand: &[u8],
    prev: &Combo,
    allow_bomb: bool,
) -> Vec<Combo> {
    super::doudizhu_ai_combo::counter_candidates(hand, prev, allow_bomb)
}

/// 领出候选：拆手牌找最不伤结构的单手。优先短小弱牌。
pub fn lead_candidates(hand: &[u8]) -> Vec<Combo> {
    let mut out = Vec::new();
    let counts = counts_of(hand);
    // 单张（从小到大）
    for r in 0..13u8 {
        if counts[r as usize] == 1 {
            if let Some(c) = hand
                .iter()
                .copied()
                .find(|&c| Rank::of(c).0 == r)
                .and_then(|x| detect(&[x]))
            {
                out.push(c);
            }
        }
    }
    // 对子
    for r in 0..13u8 {
        if counts[r as usize] == 2 {
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
    // 三张（不带，结构干净）
    for r in 0..13u8 {
        if counts[r as usize] == 3 {
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
    // 顺子(≥5)/连对(≥3)/飞机(≥2)：找 rank 连续段，多张一起出
    for (need, min_len) in [(1usize, 5usize), (2, 3), (3, 2)] {
        let mut start: Option<u8> = None;
        for r in 0..=12u8 {
            let ok = r < 12 && counts[r as usize] >= need as u8;
            if ok {
                if start.is_none() {
                    start = Some(r);
                }
            } else if let Some(s) = start.take() {
                let len = r as usize - s as usize;
                if len >= min_len {
                    let mut cards = Vec::new();
                    for rr in s..r {
                        let take: Vec<u8> = hand
                            .iter()
                            .copied()
                            .filter(|&c| Rank::of(c).0 == rr)
                            .take(need)
                            .collect();
                        cards.extend(take);
                    }
                    if let Some(c) = detect(&cards) {
                        out.push(c);
                    }
                }
            }
        }
    }
    out
}

/// AI 决策入口：返回要出的牌（None = 过牌）。
///
/// `seat` 出牌者座位，`teammate_low` = 队友手牌是否已很少（农民配合用）。
pub fn ai_choose(
    hand: &[u8],
    last: Option<(&Combo, u8)>,
    seat: u8,
    teammate_low: bool,
    landlord: Option<u8>,
) -> Option<Vec<u8>> {
    match last {
        None => {
            // 领出：残局先找一步清仓，否则出最小单张
            if let Some(fin) = finisher(hand) {
                return Some(fin.cards);
            }
            let cands = lead_candidates(hand);
            // 真人领出倾向一次甩掉长牌型，而非拆顺子出单张
            if let Some(long) =
                cands.iter().filter(|c| c.len >= 5).max_by_key(|c| c.len)
            {
                return Some(long.cards.clone());
            }
            cands.first().map(|c| c.cards.clone())
        }
        Some((prev, last_seat)) => {
            // 农民配合：只有「我和上家都不是地主」才互为队友。
            // 按座位号硬判 1↔2 会在 AI 当地主时把地主当队友，
            // 导致农民故意放过地主（阵营 bug）。
            let same_side = landlord
                .map(|l| l != seat && l != last_seat && seat != last_seat)
                .unwrap_or(false);
            if same_side && teammate_low {
                return None;
            }
            let cands = counter_candidates(hand, prev, true);
            // 残局清仓优先
            for c in &cands {
                if let Some(rest) = take(hand, &c.cards) {
                    if rest.is_empty() {
                        return Some(c.cards.clone());
                    }
                }
            }
            // 惜弹：非残局不出炸弹（候选排序里炸弹在尾部）
            let plain: Vec<&Combo> = cands
                .iter()
                .filter(|c| {
                    !matches!(c.kind, ComboKind::Bomb | ComboKind::Rocket)
                })
                .collect();
            if let Some(c) = plain.first() {
                return Some(c.cards.clone());
            }
            if hand.len() <= 4 {
                // 残局才舍得放炸弹
                cands.last().map(|c| c.cards.clone())
            } else {
                None
            }
        }
    }
}

/// 一步清仓：手牌能一手出完（领出任意合法牌型）。
pub fn finisher(hand: &[u8]) -> Option<Combo> {
    detect(hand)
}

/// 从手牌里挑出 `sel` 的合法牌型，且能压过 `prev`。手牌分离只关心牌面。
pub fn legal_pick(
    hand: &[u8],
    sel: &[u8],
    prev: Option<&Combo>,
) -> Option<Combo> {
    let known: std::collections::HashSet<u8> = hand.iter().copied().collect();
    if sel.iter().any(|c| !known.contains(c)) {
        return None;
    }
    let c = detect(sel)?;
    if !beats(&c, prev) {
        return None;
    }
    Some(c)
}
