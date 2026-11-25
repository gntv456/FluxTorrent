//! 斗地主（AI 房）：牌编码、牌型识别与比较的**纯函数**层。
//! 刻意不碰 DB、不拿时钟（同 `bs_room.rs` 纪律）；[`deal`] 内部
//! thread_rng 仅开局一次性使用。
//! 牌编码 `0..54`：`card = rank * 4 + suit`，rank 0..12 = 3..2、
//! 13/14 = 小/大王（编码 52/53）；王 > 2 > A > … > 3。
//! 经济口径（EV<1 红线）在 [`super::doudizhu_settle`]。

use rand::seq::SliceRandom;
use rand::thread_rng;

/// 派彩抽水硬上限（千分比）：设置键钳到此值，防配置把抽水配成 50% 以上。
pub const RAKE_PERMILLE_HARD_CAP: i64 = 500;
/// 缺省抽水（千分比）＝10%，与猜大小 `BIGSMALL_RAKE_PERMILLE` 同族。
pub const DOU_DIZHU_RAKE: i64 = 100;
/// 倍数封顶（经典规则：炸弹×2、王炸×4、春天×2，叫分底 1/2/3）。
pub const MULTIPLIER_CAP: i64 = 8;

/// 牌面（rank 0..12 = 3..2；13 小王；14 大王）。
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct Rank(pub u8);

impl Rank {
    /// 从牌编码取 rank（王在高位 13/14）。
    pub fn of(card: u8) -> Self {
        if card >= 52 {
            Rank(13 + (card - 52))
        } else {
            Rank(card / 4)
        }
    }

    /// 显示名（3..10、J、Q、K、A、2、王）。
    pub fn label(self) -> &'static str {
        const LABELS: [&str; 15] = [
            "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A", "2",
            "w", "W",
        ];
        LABELS.get(self.0 as usize).copied().unwrap_or("?")
    }
}

/// 发牌洗匀切三家（降序）与底牌；seat0=玩家，1/2=AI。
pub fn deal() -> ([[u8; 17]; 3], [u8; 3]) {
    let mut deck: Vec<u8> = (0..54u8).collect();
    deck.shuffle(&mut thread_rng());
    let mut hands = [[0u8; 17]; 3];
    for seat in 0..3 {
        let mut cards: Vec<u8> = deck[seat * 17..(seat + 1) * 17].to_vec();
        cards.sort_by_key(|c| std::cmp::Reverse(Rank::of(*c)));
        hands[seat].copy_from_slice(&cards);
    }
    let mut kitty = [0u8; 3];
    kitty.copy_from_slice(&deck[51..54]);
    (hands, kitty)
}

/// 出牌牌型。`cards` 保持调用方传入的编码列表（识别只用 rank 计数）。
/// Serialize/Deserialize 供对局快照落 jsonb（`doudizhu_game::GameState`）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Combo {
    pub kind: ComboKind,
    /// 主牌 rank（顺子/连对/飞机取最大端）；王炸/炸弹取自身。
    pub major: Rank,
    /// 张数（三带一带 4 张等）。
    pub len: usize,
    pub cards: Vec<u8>,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum ComboKind {
    Single,
    Pair,
    Triple,
    TripleSingle,
    TriplePair,
    Straight,
    PairStraight,
    Plane,
    PlaneSingle,
    PlanePair,
    FourTwoSingle,
    FourTwoPair,
    Bomb,
    Rocket,
}

/// 对 54 张编码做 rank 计数，识别牌型；非法组合返回 None。
/// 口径：顺子 ≥5 连续不含 2/王；连对 ≥3 对连续不含 2/王；
/// 飞机 ≥2 连续三张（不含 2/王）带同数量单/对；四带二=两单或两对。
pub fn detect(cards: &[u8]) -> Option<Combo> {
    let n = cards.len();
    if n == 0 || n > 20 {
        return None;
    }
    let mut counts = [0u8; 15];
    for &c in cards {
        counts[Rank::of(c).0 as usize] += 1;
    }
    let kinds: Vec<usize> = (0..15).filter(|&r| counts[r] > 0).collect();
    let singles = kinds.iter().filter(|&&r| counts[r] == 1).count();
    let pairs = kinds.iter().filter(|&&r| counts[r] == 2).count();
    let triples: Vec<usize> =
        kinds.iter().copied().filter(|&r| counts[r] == 3).collect();
    let bombs: Vec<usize> =
        kinds.iter().copied().filter(|&r| counts[r] == 4).collect();

    // 王炸（双王）
    if n == 2 && counts[13] == 1 && counts[14] == 1 {
        return Some(combo(ComboKind::Rocket, Rank(14), cards));
    }
    if kinds.len() != 1 {
        if let Some(c) = compound(
            n, &counts, &kinds, &triples, &bombs, singles, pairs, cards,
        ) {
            return Some(c);
        }
        return None;
    }
    let r = kinds[0];
    match counts[r] {
        1 => Some(combo(ComboKind::Single, Rank(r as u8), cards)),
        2 => Some(combo(ComboKind::Pair, Rank(r as u8), cards)),
        3 => Some(combo(ComboKind::Triple, Rank(r as u8), cards)),
        4 => Some(combo(ComboKind::Bomb, Rank(r as u8), cards)),
        _ => None,
    }
}

/// 复合牌型（顺/连对/飞机/三带/四带二）识别。
#[allow(clippy::too_many_arguments)]
fn compound(
    n: usize,
    counts: &[u8; 15],
    kinds: &[usize],
    triples: &[usize],
    bombs: &[usize],
    singles: usize,
    pairs: usize,
    cards: &[u8],
) -> Option<Combo> {
    // 三带一 / 三带二
    if triples.len() == 1 && bombs.is_empty() {
        if n == 4 && singles == 1 {
            let t = triples[0];
            return Some(combo(ComboKind::TripleSingle, Rank(t as u8), cards));
        }
        if n == 5 && pairs == 1 {
            let t = triples[0];
            return Some(combo(ComboKind::TriplePair, Rank(t as u8), cards));
        }
    }
    // 四带二（两张单或两对；带牌不含王炸组合也可以，只按张数判）
    if bombs.len() == 1 && triples.is_empty() {
        if n == 6 && singles == 2 {
            let b = bombs[0];
            return Some(combo(ComboKind::FourTwoSingle, Rank(b as u8), cards));
        }
        if n == 8 && pairs == 2 {
            let b = bombs[0];
            return Some(combo(ComboKind::FourTwoPair, Rank(b as u8), cards));
        }
    }
    // 纯顺子 / 纯连对（kinds.len() 可能只有 3，不能直接下标 [4]）
    if bombs.is_empty() && triples.is_empty() && is_run(kinds) {
        let top = kinds[kinds.len() - 1];
        if n == kinds.len() && kinds.len() >= 5 && top < 12 {
            return Some(combo(ComboKind::Straight, Rank(top as u8), cards));
        }
        if n == kinds.len() * 2
            && pairs == kinds.len()
            && kinds.len() >= 3
            && top < 12
        {
            return Some(combo(
                ComboKind::PairStraight,
                Rank(top as u8),
                cards,
            ));
        }
    }
    plane(n, counts, kinds, triples, singles, cards)
}

/// 飞机：≥2 个连续三张（不含 2/王），带同数量单张或对子；允许连续段里
/// 挑一个子集做「机身」（经典 33344455 带三张 = 机身 334455 之类），
/// 这里取「最长连续段作为机身」的简化口径，机身端点做比较基准。
#[allow(clippy::too_many_arguments)]
fn plane(
    n: usize,
    counts: &[u8; 15],
    kinds: &[usize],
    triples: &[usize],
    singles: usize,
    cards: &[u8],
) -> Option<Combo> {
    if triples.len() < 2 || kinds[kinds.len() - 1] >= 12 || !bombs_free(counts)
    {
        return None;
    }
    let mut best: Option<(usize, usize)> = None; // (start, len)
    let mut i = 0;
    while i < triples.len() {
        let mut j = i;
        while j + 1 < triples.len() && triples[j + 1] == triples[j] + 1 {
            j += 1;
        }
        let len = j - i + 1;
        if best.is_none_or(|(_, bl)| len > bl) {
            best = Some((i, len));
        }
        i = j + 1;
    }
    let (start, len) = best?;
    if len < 2 {
        return None;
    }
    let body = len * 3;
    let wing = n - body;
    let major = Rank(triples[start + len - 1] as u8);
    if wing == 0 && n == body {
        return Some(combo(ComboKind::Plane, major, cards));
    }
    if wing == len && singles == len {
        return Some(combo(ComboKind::PlaneSingle, major, cards));
    }
    if wing == len * 2 && singles == 0 {
        // 带对：剩余张全部由对子构成（每 rank 计数为偶数）。不做全局
        // pairs==len 计数——机身相邻 rank 的 44 在 3334445566 里也凑成
        // 对，会把 pairs 顶到 3 而误拒标准牌型（资深玩家实测抓到的）。
        let pairs_ok = kinds
            .iter()
            .all(|&r| counts[r] == 3 || counts[r].is_multiple_of(2));
        if pairs_ok {
            return Some(combo(ComboKind::PlanePair, major, cards));
        }
    }
    None
}

fn bombs_free(counts: &[u8; 15]) -> bool {
    counts.iter().all(|&c| c < 4)
}

/// kinds 是否连续递增。
fn is_run(kinds: &[usize]) -> bool {
    kinds.windows(2).all(|w| w[1] == w[0] + 1)
}

fn combo(kind: ComboKind, major: Rank, cards: &[u8]) -> Combo {
    Combo {
        kind,
        major,
        len: cards.len(),
        cards: cards.to_vec(),
    }
}

/// `a` 能否压过 `b`；`b` 为 None（领出）时任何合法牌型都可出。
pub fn beats(a: &Combo, b: Option<&Combo>) -> bool {
    let Some(prev) = b else {
        return true;
    };
    match (a.kind, prev.kind) {
        (ComboKind::Rocket, _) => true,
        (_, ComboKind::Rocket) => false,
        (ComboKind::Bomb, ComboKind::Bomb) => a.major > prev.major,
        (ComboKind::Bomb, _) => true,
        (_, ComboKind::Bomb) => false,
        // 同型比大小；异型不可压
        (ak, pk) if ak == pk => a.len == prev.len && a.major > prev.major,
        _ => false,
    }
}
