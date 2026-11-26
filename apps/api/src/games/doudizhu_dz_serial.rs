//! 斗地主 DouZero 连型枚举工具（上游 `move_generator.py` 内部件拆出，
//! 守 300 行门禁）：连续段切分、滑窗生成、rank 组合选取。

use super::doudizhu_dz_move_gen::rank_counts;

/// 组合选取 C(n, k)（上游 `itertools.combinations`，输入须升序去重）。
pub(super) fn combinations(pool: &[i16], k: usize) -> Vec<Vec<i16>> {
    let mut out = Vec::new();
    if k == 0 {
        out.push(Vec::new());
        return out;
    }
    if pool.len() < k {
        return out;
    }
    let mut idx: Vec<usize> = (0..k).collect();
    loop {
        out.push(idx.iter().map(|&i| pool[i]).collect());
        // 词典序推进：最右还能涨的位置（idx[i-1] 上限 = n-k+i-1）
        let mut i = k;
        while i > 0 && idx[i - 1] >= pool.len() - k + i - 1 {
            i -= 1;
        }
        if i == 0 {
            break;
        }
        idx[i - 1] += 1;
        for j in i..k {
            idx[j] = idx[j - 1] + 1;
        }
    }
    out
}

/// 长度 ≥`min_serial`（或恰 `repeat_num`）的全部连续组合。
pub(super) fn gen_serial(
    pool: &[i16],
    min_serial: usize,
    repeat: usize,
    repeat_num: usize,
) -> Vec<Vec<i16>> {
    let mut moves = Vec::new();
    for (start, longest) in seq_records(pool) {
        if longest < min_serial {
            continue;
        }
        let seg = &pool[start..start + longest];
        if repeat_num == 0 {
            let mut steps = min_serial;
            while steps <= longest {
                for idx in 0..=(longest - steps) {
                    let mut mv = Vec::with_capacity(steps * repeat);
                    for &r in &seg[idx..idx + steps] {
                        for _ in 0..repeat {
                            mv.push(r);
                        }
                    }
                    moves.push(mv);
                }
                steps += 1;
            }
        } else {
            if longest < repeat_num {
                continue;
            }
            for idx in 0..=(longest - repeat_num) {
                let mut mv = Vec::with_capacity(repeat_num * repeat);
                for &r in &seg[idx..idx + repeat_num] {
                    for _ in 0..repeat {
                        mv.push(r);
                    }
                }
                moves.push(mv);
            }
        }
    }
    moves
}

/// 单调连续段切分（rank 升序列表 → (起点, 长度) 列表）。
pub(super) fn seq_records(ranks: &[i16]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut len = 1usize;
    for i in 1..=ranks.len() {
        if i < ranks.len() && ranks[i] == ranks[i - 1] + 1 {
            len += 1;
        } else {
            out.push((start, len));
            start = i;
            len = 1;
        }
    }
    out
}

/// 手牌里存在的去重 rank 升序表。
pub(super) fn sorted_ranks(cards: &[i16]) -> Vec<i16> {
    let mut v: Vec<i16> = rank_counts(cards).keys().copied().collect();
    v.sort_unstable();
    v
}
