//! 斗地主 DouZero 全量合法出牌枚举器（Apache-2.0 的 kwai/DouZero
//! `move_generator.py` 移植）。与 `doudizhu_ai_combo` 的「最小压制候选」
//! 不同，这里枚举**全部**合法响应与领出——神经网络估值需要完整动作集。
//!
//! rank 口径：DouZero 3..14,17（普通）+ 20 小王/30 大王；本仓 0..12,13,14。
//! 枚举按 DouZero rank（`Rank(r)` 即本仓 rank+3 映射）推进，返回
//! DouZero rank 列表；调用方经 [`dz_cards_from_ranks`] 换回实体牌编码。
//!
//! 移植口径与上游逐条对齐（防「自作主张改规则」漂移）：
//! - 顺子 ≥5 连续不含 2（rank 14）/王；连对 ≥3 对；飞机 ≥2 连三张；
//! - 三带一/三带二翅膀不得与机身同 rank；飞机翅膀按 rank 组合选取；
//! - 四带二带两单（可拆对成两张单）或两对；王炸 [20,30]；
//! - 响应过滤与上游 `move_selector.py` 一致（含 3带1 取 move[1]、
//!   3带2 取 move[2] 的排序位口径）。

use std::collections::HashMap;

use super::doudizhu_dz_serial::{
    combinations, gen_serial, seq_records, sorted_ranks,
};

/// DouZero rank 常量（避免魔数满天飞）。
pub(super) const R2: i16 = 14; // 本仓 rank 11
pub(super) const RJOKER_SM: i16 = 20; // 小王（本仓 13）
pub(super) const RJOKER_BG: i16 = 30; // 大王（本仓 14）

/// 最小长度（上游 utils.py）。
const MIN_SERIAL: usize = 5;
const MIN_PAIRS: usize = 3;
const MIN_TRIPLES: usize = 2;

/// 牌型代号（上游 TYPE_* 数值，响应过滤按它分派）。
pub(super) const T_PASS: u8 = 0;
pub(super) const T_SINGLE: u8 = 1;
pub(super) const T_PAIR: u8 = 2;
pub(super) const T_TRIPLE: u8 = 3;
pub(super) const T_BOMB: u8 = 4;
pub(super) const T_KING_BOMB: u8 = 5;
pub(super) const T_3_1: u8 = 6;
pub(super) const T_3_2: u8 = 7;
pub(super) const T_SERIAL_SINGLE: u8 = 8;
pub(super) const T_SERIAL_PAIR: u8 = 9;
pub(super) const T_SERIAL_TRIPLE: u8 = 10;
pub(super) const T_SERIAL_3_1: u8 = 11;
pub(super) const T_SERIAL_3_2: u8 = 12;
pub(super) const T_4_2: u8 = 13;
pub(super) const T_4_22: u8 = 14;
pub(super) const T_WRONG: u8 = 15;

/// 手牌 rank 计数（DouZero rank → 张数）。
pub(super) fn rank_counts(cards: &[i16]) -> HashMap<i16, u8> {
    let mut m: HashMap<i16, u8> = HashMap::new();
    for &c in cards {
        *m.entry(c).or_insert(0) += 1;
    }
    m
}

/// 上游 `_gen_serial_moves`：从可连 rank 池生成 `repeat` 张/组、

/// 全量领出/响应动作枚举器（对应上游 `MovesGener`）。
pub(super) struct MoveGen<'a> {
    cards: &'a [i16],
    counts: HashMap<i16, u8>,
}

impl<'a> MoveGen<'a> {
    pub(super) fn new(cards: &'a [i16]) -> Self {
        MoveGen {
            cards,
            counts: rank_counts(cards),
        }
    }

    pub(super) fn ranks_with(&self, min: u8) -> Vec<i16> {
        let mut v: Vec<i16> = self
            .counts
            .iter()
            .filter(|(_, &n)| n >= min)
            .map(|(&r, _)| r)
            .collect();
        v.sort_unstable();
        v
    }

    pub(super) fn gen_singles(&self) -> Vec<Vec<i16>> {
        sorted_ranks(self.cards)
            .into_iter()
            .map(|r| vec![r])
            .collect()
    }

    pub(super) fn gen_pairs(&self) -> Vec<Vec<i16>> {
        self.ranks_with(2).into_iter().map(|r| vec![r, r]).collect()
    }

    pub(super) fn gen_triples(&self) -> Vec<Vec<i16>> {
        self.ranks_with(3)
            .into_iter()
            .map(|r| vec![r, r, r])
            .collect()
    }

    pub(super) fn gen_bombs(&self) -> Vec<Vec<i16>> {
        self.ranks_with(4)
            .into_iter()
            .map(|r| vec![r, r, r, r])
            .collect()
    }

    pub(super) fn gen_king_bomb(&self) -> Vec<Vec<i16>> {
        let has = |r: i16| self.counts.get(&r).copied().unwrap_or(0) > 0;
        if has(RJOKER_SM) && has(RJOKER_BG) {
            vec![vec![RJOKER_SM, RJOKER_BG]]
        } else {
            Vec::new()
        }
    }

    pub(super) fn gen_3_1(&self) -> Vec<Vec<i16>> {
        let triples = self.gen_triples();
        let singles = self.gen_singles();
        let mut out = Vec::new();
        for t in &triples {
            for s in &singles {
                if s[0] != t[0] {
                    let mut mv = t.clone();
                    mv.push(s[0]);
                    out.push(mv);
                }
            }
        }
        out
    }

    pub(super) fn gen_3_2(&self) -> Vec<Vec<i16>> {
        let triples = self.gen_triples();
        let pairs = self.gen_pairs();
        let mut out = Vec::new();
        for t in &triples {
            for p in &pairs {
                if p[0] != t[0] {
                    let mut mv = t.clone();
                    mv.extend_from_slice(p);
                    out.push(mv);
                }
            }
        }
        out
    }

    pub(super) fn gen_serial_single(&self, repeat_num: usize) -> Vec<Vec<i16>> {
        // 顺子池 = 全部普通牌 rank（≤R2，不含王）
        let pool: Vec<i16> = sorted_ranks(self.cards)
            .into_iter()
            .filter(|&r| r <= R2)
            .collect();
        gen_serial(&pool, MIN_SERIAL, 1, repeat_num)
    }

    pub(super) fn gen_serial_pair(&self, repeat_num: usize) -> Vec<Vec<i16>> {
        let pool: Vec<i16> = self
            .ranks_with(2)
            .into_iter()
            .filter(|&r| r <= R2)
            .collect();
        gen_serial(&pool, MIN_PAIRS, 2, repeat_num)
    }

    pub(super) fn gen_serial_triple(&self, repeat_num: usize) -> Vec<Vec<i16>> {
        let pool: Vec<i16> = self
            .ranks_with(3)
            .into_iter()
            .filter(|&r| r <= R2)
            .collect();
        gen_serial(&pool, MIN_TRIPLES, 3, repeat_num)
    }

    pub(super) fn gen_serial_3_1(&self, repeat_num: usize) -> Vec<Vec<i16>> {
        let mut out = Vec::new();
        for s3 in self.gen_serial_triple(repeat_num) {
            let body: Vec<i16> = {
                let mut v: Vec<i16> = s3.clone();
                v.dedup();
                v
            };
            // 翅膀池：非机身 rank 的全部实体牌（含第 4 张——上游按实体剔除）
            let wings: Vec<i16> = self
                .cards
                .iter()
                .copied()
                .filter(|c| !body.contains(c))
                .collect();
            // 上游对实体牌再 set 去重后做组合选取
            let mut wing_pool = sorted_ranks(&wings);
            wing_pool.dedup();
            for combo in combinations(&wing_pool, body.len()) {
                let mut mv = s3.clone();
                mv.extend(combo);
                mv.sort_unstable();
                out.push(mv);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub(super) fn gen_serial_3_2(&self, repeat_num: usize) -> Vec<Vec<i16>> {
        let mut out = Vec::new();
        let pair_pool = self.ranks_with(2);
        for s3 in self.gen_serial_triple(repeat_num) {
            let body: Vec<i16> = {
                let mut v: Vec<i16> = s3.clone();
                v.dedup();
                v
            };
            let cands: Vec<i16> = pair_pool
                .iter()
                .copied()
                .filter(|r| !body.contains(r))
                .collect();
            for combo in combinations(&cands, body.len()) {
                let mut mv = s3.clone();
                for &r in &combo {
                    mv.extend([r, r]);
                }
                mv.sort_unstable();
                out.push(mv);
            }
        }
        out
    }

    pub(super) fn gen_4_2(&self) -> Vec<Vec<i16>> {
        let mut out = Vec::new();
        for fc in self.ranks_with(4) {
            // 翅膀 = 剔除四张实体后的任意两张（可同 rank 拆对）
            let wings: Vec<i16> =
                self.cards.iter().copied().filter(|&c| c != fc).collect();
            let mut pool = sorted_ranks(&wings);
            pool.dedup();
            for combo in combinations(&pool, 2) {
                let mut mv = vec![fc; 4];
                mv.extend(combo);
                out.push(mv);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    pub(super) fn gen_4_22(&self) -> Vec<Vec<i16>> {
        let mut out = Vec::new();
        for fc in self.ranks_with(4) {
            let cands: Vec<i16> = self
                .ranks_with(2)
                .into_iter()
                .filter(|&r| r != fc)
                .collect();
            for combo in combinations(&cands, 2) {
                let mut mv = vec![fc; 4];
                for &r in &combo {
                    mv.extend([r, r]);
                }
                out.push(mv);
            }
        }
        out
    }

    /// 全部牌型（领出用；上游 `gen_moves`）。
    pub(super) fn gen_all(&self) -> Vec<Vec<i16>> {
        let mut moves: Vec<Vec<i16>> = Vec::new();
        moves.extend(self.gen_singles());
        moves.extend(self.gen_pairs());
        moves.extend(self.gen_triples());
        moves.extend(self.gen_bombs());
        moves.extend(self.gen_king_bomb());
        moves.extend(self.gen_3_1());
        moves.extend(self.gen_3_2());
        moves.extend(self.gen_serial_single(0));
        moves.extend(self.gen_serial_pair(0));
        moves.extend(self.gen_serial_triple(0));
        moves.extend(self.gen_serial_3_1(0));
        moves.extend(self.gen_serial_3_2(0));
        moves.extend(self.gen_4_2());
        moves.extend(self.gen_4_22());
        moves
    }
}
