//! 斗地主癞子模式：癞子选定与牌型判定变体。
//!
//! 规则口径（经典癞子/癞子斗地主）：翻出底牌最后一张，其点数即「癞子点」，
//! 该点数的 4 张牌全部成为万能牌——可当作任意点数（仍不可作王）参与
//! 组合。癞子不改变牌型比较（major/len 仍按面值），只改「一组牌能否
//! 组成某牌型」的判定面。
//!
//! 判定实现：`effective_combos` 枚举癞子的替身点数；癞子不可区分，
//! 按非递减序枚举组合（4 张癞子从 13^4=28561 降到 C(16,4)=1820），
//! 逐个走经典 `detect`，去重后返回「这组牌可能构成的全部牌型」。
//! 压制判定取「能压过 prev 的最大面」——与经典同型比较一致。

use super::doudizhu::{detect, Combo, ComboKind, Rank};

/// 底牌最后一张的点数 = 癞子点。王不能当癞子（无同点四张），
/// 遇到王回退到 rank 0（3），避免癞子桌 wildcards 全空。
pub fn wild_rank_card(kitty_last: u8) -> u8 {
    let r = Rank::of(kitty_last).0;
    if r >= 13 {
        0
    } else {
        r
    }
}

/// 从一手牌里找出癞子点数的牌（即本座的癞子）。
pub fn seat_wildcards(hand: &[u8], wild_rank: u8) -> Vec<u8> {
    hand.iter()
        .copied()
        .filter(|&c| Rank::of(c).0 == wild_rank && c < 52)
        .collect()
}

/// 三座手牌 + 底牌 → 各座癞子编码表（`WildMap` 形状）。
pub fn pick_wildcards(hands: &[Vec<u8>; 3], kitty: &[u8]) -> [Vec<u8>; 3] {
    // 倒序找最后一张非王作为癞子点；底牌全王则 wild_rank_card 回退到 3。
    let last = kitty
        .iter()
        .rev()
        .find(|&&c| c < 52)
        .copied()
        .or_else(|| kitty.last().copied());
    let Some(last) = last else {
        return [Vec::new(), Vec::new(), Vec::new()];
    };
    let wr = wild_rank_card(last);
    [
        seat_wildcards(&hands[0], wr),
        seat_wildcards(&hands[1], wr),
        seat_wildcards(&hands[2], wr),
    ]
}

/// 判定用的替身点数候选：癞子可当 3..A、2（王不可当），也可当「不参与
/// 结构」（保持原点数参与判型——detect 本来就按原点数算）。
const WILD_TARGETS: [u8; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/// 递归枚举癞子替身（每个癞子独立选点数），对每个组合跑经典 detect，
/// 收集全部可行牌型；`major` 去重后保留每 (kind, len) 下面值最大者。
pub fn effective_combos(sel: &[u8], wilds: &[u8]) -> Vec<Combo> {
    if wilds.is_empty() {
        return detect(sel).into_iter().collect();
    }
    let wild_set: std::collections::HashSet<u8> =
        wilds.iter().copied().collect();
    let plain: Vec<u8> = sel
        .iter()
        .copied()
        .filter(|c| !wild_set.contains(c))
        .collect();
    let n_wild = sel.iter().filter(|c| wild_set.contains(c)).count();
    let mut out: Vec<Combo> = Vec::new();
    let mut assigned = vec![0u8; n_wild];
    // 癞子之间不可区分：detect 只看 rank 计数，替身分配按非递减顺序
    // 枚举即可覆盖全部牌型，从 13^k 排列降为 C(13+k-1,k) 组合。
    enumerate_wild(&plain, 0, 0, &mut assigned, &mut out);
    // 每 (kind, len) 保留 major 最大的一个（比较面最宽）
    out.sort_by(|a, b| {
        (a.kind, a.len)
            .cmp(&(b.kind, b.len))
            .then(b.major.cmp(&a.major))
    });
    out.dedup_by(|a, b| a.kind == b.kind && a.len == b.len);
    out
}

#[allow(clippy::too_many_arguments)]
fn enumerate_wild(
    plain: &[u8],
    depth: usize,
    min_target: usize,
    assigned: &mut [u8],
    out: &mut Vec<Combo>,
) {
    if depth == assigned.len() {
        // 组装虚拟手牌：plain 原编码 + 癞子替换成目标点数的一张实体牌
        let mut virtual_sel = plain.to_vec();
        for &target in assigned.iter() {
            // 找一手里该点数的一张非癞子实体作为替身编码不可行（可能
            // 手里没有）——直接构造编码：rank*4（花色无所谓，detect 只看 rank）
            virtual_sel.push(target * 4);
        }
        // 替身编码必须存在于原 sel 集合语义里；detect 只用 rank 计数，
        // 这里保证数量一致即可
        if let Some(c) = detect(&virtual_sel) {
            out.push(c);
        }
        return;
    }
    // 非递减：只枚举 target >= min_target，避免同一多重集被重复排列
    for (i, &target) in WILD_TARGETS.iter().enumerate().skip(min_target) {
        assigned[depth] = target;
        enumerate_wild(plain, depth + 1, i, assigned, out);
    }
}

/// 癞子模式下「sel 能否压过 prev」：取全部有效牌型里任一能压即可。
/// 领出（prev=None）时任何有效牌型都行。
pub fn wild_beats(
    sel: &[u8],
    wilds: &[u8],
    prev: Option<&Combo>,
) -> Option<Combo> {
    let candidates = effective_combos(sel, wilds);
    match prev {
        None => candidates.into_iter().next(),
        Some(p) => candidates
            .into_iter()
            .filter(|c| super::doudizhu::beats(c, Some(p)))
            .max_by(|a, b| a.major.cmp(&b.major)),
    }
}

/// 癞子模式的提示：返回一手能压过 prev 的最小出牌（原牌编码）。
///
/// 从手牌按 rank 聚合出「单/对/三/炸」基础候选，再由癞子补齐；
/// 逐个走 wild_beats 枚举替身面，取首个（最小）合法解。
/// 不搜索顺子/连对/飞机等长牌型——提示只求「有牌可出」，
/// 与 AI 的氛围组定位一致。
pub fn wild_hint(
    hand: &[u8],
    wilds: &[u8],
    prev: Option<&Combo>,
) -> Option<Vec<u8>> {
    let wild_set: std::collections::HashSet<u8> =
        wilds.iter().copied().collect();
    let mut by_rank: [Vec<u8>; 15] = std::array::from_fn(|_| Vec::new());
    for &c in hand {
        if !wild_set.contains(&c) {
            by_rank[super::doudizhu::Rank::of(c).0 as usize].push(c);
        }
    }
    // 纯癞子候选：单张癞子可变任意点（如癞子当 A 压 K）。
    // wild_beats 只做合法/压制判定，这里返回原始手牌编码供 play() 使用。
    for wild_n in 1..=wilds.len().min(4) {
        let sel = wilds[..wild_n].to_vec();
        if wild_beats(&sel, &sel, prev).is_some() {
            return Some(sel);
        }
    }
    // 按 rank 从小到大枚举「实体牌 + 癞子补齐」的组合
    for r in 0..15u8 {
        let plain = &by_rank[r as usize];
        for take_n in 1..=plain.len().min(4) {
            for wild_n in 0..=wilds.len().min(4 - take_n) {
                let mut sel = plain[..take_n].to_vec();
                sel.extend_from_slice(&wilds[..wild_n]);
                let ws = wilds[..wild_n].to_vec();
                if wild_beats(&sel, &ws, prev).is_some() {
                    return Some(sel);
                }
            }
        }
    }
    None
}

/// 炸弹判定（癞子参与）：四张同点（含癞子替身）已是 detect 面；
/// 王炸仍是双王（癞子不改变王炸——癞子点数的牌不是王）。
pub fn is_bomb_like(kind: ComboKind) -> bool {
    matches!(kind, ComboKind::Bomb | ComboKind::Rocket)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(rank: u8, suit: usize) -> u8 {
        rank * 4 + suit as u8
    }

    #[test]
    fn seat_wildcards_picks_same_rank() {
        // 癞子点 = rank 5（牌编码 20..23）
        let hand = vec![c(5, 0), c(5, 1), c(2, 2), c(9, 3)];
        let wilds = seat_wildcards(&hand, 5);
        assert_eq!(wilds, vec![c(5, 0), c(5, 1)]);
    }

    #[test]
    fn wild_pair_from_single_and_wild() {
        // 手牌：单 5(癞子) + 单 9；癞子当 9 → 对 9
        let hand = vec![c(5, 0), c(9, 1)];
        let wilds = vec![c(5, 0)];
        let sel = [c(5, 0), c(9, 1)];
        let combos = effective_combos(&sel, &wilds);
        assert!(combos
            .iter()
            .any(|k| k.kind == ComboKind::Pair && k.major == Rank(9)));
    }

    #[test]
    fn wild_triple_via_two_wilds() {
        // 两张癞子(点5) + 一张 K(11) → 三张 K
        let hand = vec![c(5, 0), c(5, 1), c(11, 2)];
        let wilds = vec![c(5, 0), c(5, 1)];
        let sel = &hand;
        let combos = effective_combos(sel, &wilds);
        assert!(combos
            .iter()
            .any(|k| k.kind == ComboKind::Triple && k.major == Rank(11)));
    }

    #[test]
    fn wild_bomb_via_four() {
        // 3 癞子 + 1 张 2 → 四张 2 炸弹
        let hand = vec![c(5, 0), c(5, 1), c(5, 2), c(12, 3)];
        let wilds = vec![c(5, 0), c(5, 1), c(5, 2)];
        let combos = effective_combos(&hand, &wilds);
        assert!(combos
            .iter()
            .any(|k| k.kind == ComboKind::Bomb && k.major == Rank(12)));
    }

    #[test]
    fn wild_bomb_via_four_plus_wilds_only() {
        // 4 张癞子（点5）→ 四张 A 炸弹（全替身）
        let hand = vec![c(5, 0), c(5, 1), c(5, 2), c(5, 3)];
        let wilds = &hand;
        // 癞子点 5 的 4 张全替身：最高可当 2（rank12），2 > A
        let combos = effective_combos(&hand, &wilds);
        assert!(combos
            .iter()
            .any(|k| k.kind == ComboKind::Bomb && k.major == Rank(12)));
    }

    #[test]
    fn wild_beats_finds_min_or_none() {
        let hand = vec![c(5, 0), c(5, 1), c(2, 2)];
        let wilds = vec![c(5, 0), c(5, 1)];
        // prev = 单 K(9)
        let prev = detect(&[c(9, 0)]).unwrap();
        // 癞子当 A 压过
        let hit = wild_beats(&[c(5, 0)], &wilds[..2], Some(&prev));
        // sel 只有一张癞子——需把 wilds 限定到 sel 里的那张
        let hit = wild_beats(&[c(5, 0)], &[c(5, 0)], Some(&prev));
        assert!(hit.is_some());
        // prev = 单 2（12），癞子最高只能到 A(11) 压不过
        let prev2 = detect(&[c(12, 0)]).unwrap();
        let miss = wild_beats(&[c(5, 0)], &[c(5, 0)], Some(&prev2));
        assert!(miss.is_none());
    }

    #[test]
    fn classic_detect_still_works_without_wilds() {
        let hand = vec![c(3, 0), c(3, 1)];
        let combos = effective_combos(&hand, &[]);
        assert!(combos
            .iter()
            .any(|k| k.kind == ComboKind::Pair && k.major == Rank(3)));
    }
}
