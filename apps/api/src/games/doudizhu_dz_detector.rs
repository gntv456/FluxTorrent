//! 斗地主 DouZero 牌型判定（上游 `move_detector.py` 移植）。
//! 从 doudizhu_dz_move_gen.rs 拆出守 300 行门禁。
//! 返回 (类型, 主 rank, 长度)；长度仅连型有义；非法 = T_WRONG。

use super::doudizhu_dz_move_gen::{
    rank_counts, RJOKER_BG, RJOKER_SM, T_3_1, T_3_2, T_4_2, T_4_22, T_BOMB,
    T_KING_BOMB, T_PAIR, T_PASS, T_SERIAL_3_1, T_SERIAL_3_2, T_SERIAL_PAIR,
    T_SERIAL_SINGLE, T_SERIAL_TRIPLE, T_SINGLE, T_TRIPLE, T_WRONG,
};

use std::collections::HashMap;

/// 飞机最小连三组数（上游 utils.py MIN_TRIPLES）。
const MIN_TRIPLES: usize = 2;

/// 牌型判定（上游 `move_detector.get_move_type` 的 Rust 版）。
/// 返回 (类型, 主 rank, 长度)；长度仅连型有义。
pub(crate) fn move_type(mv: &[i16]) -> (u8, i16, usize) {
    let n = mv.len();
    if n == 0 {
        return (T_PASS, 0, 0);
    }
    // 非法牌型一律 WRONG（上游 TYPE_15_WRONG），selector 按上游口径拼兜底
    let mut mv = mv.to_vec();
    mv.sort_unstable();
    let counts = rank_counts(&mv);
    if n == 1 {
        return (T_SINGLE, mv[0], 1);
    }
    if n == 2 {
        if mv[0] == mv[1] {
            return (T_PAIR, mv[0], 1);
        }
        if mv == [RJOKER_SM, RJOKER_BG] {
            return (T_KING_BOMB, 0, 1);
        }
        return (T_WRONG, 0, 0); // 上游 TYPE_15_WRONG
    }
    if n == 3 {
        if counts.len() == 1 {
            return (T_TRIPLE, mv[0], 1);
        }
        return (T_WRONG, 0, 0);
    }
    if n == 4 {
        if counts.len() == 1 {
            return (T_BOMB, mv[0], 1);
        }
        if counts.len() == 2 && mv[0] == mv[1] && mv[1] == mv[2]
            || counts.len() == 2 && mv[1] == mv[2] && mv[2] == mv[3]
        {
            return (T_3_1, mv[1], 1);
        }
        return (T_WRONG, 0, 0);
    }
    if is_continuous(&mv) {
        return (T_SERIAL_SINGLE, mv[0], n);
    }
    if n == 5 && counts.len() == 2 {
        return (T_3_2, mv[2], 1);
    }
    let mut type_hist: HashMap<u8, u8> = HashMap::new();
    for c in counts.values() {
        *type_hist.entry(*c).or_insert(0) += 1;
    }
    if n == 6
        && (counts.len() == 2 || counts.len() == 3)
        && type_hist.get(&4).copied() == Some(1)
        && (type_hist.get(&2).copied() == Some(1)
            || type_hist.get(&1).copied() == Some(2))
    {
        return (T_4_2, mv[2], 1);
    }
    if n == 8
        && (((counts.len() == 3 || counts.len() == 2)
            && type_hist.get(&4).copied() == Some(1)
            && type_hist.get(&2).copied() == Some(2))
            || type_hist.get(&4).copied() == Some(2))
    {
        let major = counts
            .iter()
            .filter(|(_, &c)| c == 4)
            .map(|(&r, _)| r)
            .max()
            .unwrap_or(0);
        return (T_4_22, major, 1);
    }
    let keys: Vec<i16> = {
        let mut v: Vec<i16> = counts.keys().copied().collect();
        v.sort_unstable();
        v
    };
    if counts.len() == type_hist.get(&2).copied().unwrap_or(0) as usize
        && is_continuous(&keys)
    {
        return (T_SERIAL_PAIR, keys[0], keys.len());
    }
    if counts.len() == type_hist.get(&3).copied().unwrap_or(0) as usize
        && is_continuous(&keys)
    {
        return (T_SERIAL_TRIPLE, keys[0], keys.len());
    }
    if type_hist.get(&3).copied().unwrap_or(0) >= MIN_TRIPLES as u8 {
        let mut serial3 = Vec::new();
        let mut singles = Vec::new();
        let mut pairs = Vec::new();
        for (k, v) in &counts {
            match v {
                3 => serial3.push(*k),
                1 => singles.push(*k),
                2 => pairs.push(*k),
                _ => return (T_WRONG, 0, 0),
            }
        }
        serial3.sort_unstable();
        if is_continuous(&serial3) {
            if serial3.len() == singles.len() + pairs.len() * 2 {
                return (T_SERIAL_3_1, serial3[0], serial3.len());
            }
            if serial3.len() == pairs.len() && counts.len() == serial3.len() * 2
            {
                return (T_SERIAL_3_2, serial3[0], serial3.len());
            }
        }
        if serial3.len() == 4 {
            if is_continuous(&serial3[1..]) {
                return (T_SERIAL_3_1, serial3[1], serial3.len() - 1);
            }
            if is_continuous(&serial3[..serial3.len() - 1]) {
                return (T_SERIAL_3_1, serial3[0], serial3.len() - 1);
            }
        }
    }
    (T_WRONG, 0, 0)
}

/// 升序 rank 列表是否逐项连续。
pub(super) fn is_continuous(ranks: &[i16]) -> bool {
    ranks.windows(2).all(|w| w[1] == w[0] + 1)
}
