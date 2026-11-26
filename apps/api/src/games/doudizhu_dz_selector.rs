//! 斗地主 DouZero 合法响应过滤（上游 `move_selector.py` 移植）：
//! 枚举出的同型候选按上游口径过滤「压得过 rival」的子集。
//! 口径逐条对齐上游（3带1 取 move[1]、3带2/4带2 取 move[2] 的排序位；
//! 飞机带牌取三张组的最大 rank 比较）。

use super::doudizhu_dz_detector::move_type;
use super::doudizhu_dz_move_gen::{
    rank_counts, T_3_1, T_3_2, T_4_2, T_4_22, T_BOMB, T_KING_BOMB, T_PAIR,
    T_PASS, T_SERIAL_3_1, T_SERIAL_3_2, T_SERIAL_PAIR, T_SERIAL_SINGLE,
    T_SERIAL_TRIPLE, T_SINGLE, T_TRIPLE, T_WRONG,
};

/// rival 牌能否被 `mv` 压过（同型比主 rank；炸弹/王炸兜底由调用方拼装）。
pub(super) fn beats_move(mv: &[i16], rival: &[i16]) -> bool {
    let (rty, rrank, rlen) = move_type(rival);
    let (mty, mrank, mlen) = move_type(mv);
    match (mty, rty) {
        (T_PASS, _) | (_, T_PASS) => false,
        (T_KING_BOMB, T_KING_BOMB) => false,
        (T_KING_BOMB, _) => true,
        (_, T_KING_BOMB) => false,
        (T_BOMB, T_BOMB) => mrank > rrank,
        (T_BOMB, _) => true,
        (_, T_BOMB) => false,
        (m, r) if m == r => {
            mlen == rlen && major_of(mv, m) > major_of(rival, r)
        }
        _ => false,
    }
}

/// 按牌型取比较主 rank（与上游 filter 口径一致）。
fn major_of(mv: &[i16], ty: u8) -> i16 {
    let mut sorted = mv.to_vec();
    sorted.sort_unstable();
    match ty {
        T_3_1 | T_4_2 => sorted[1],
        T_3_2 => sorted[2],
        T_SERIAL_3_1 | T_SERIAL_3_2 => {
            // 三张组里的最大 rank（上游 max(counter==3)）
            let counts = rank_counts(mv);
            counts
                .iter()
                .filter(|(_, &c)| c == 3)
                .map(|(&r, _)| r)
                .max()
                .unwrap_or(0)
        }
        T_4_22 => {
            let counts = rank_counts(mv);
            counts
                .iter()
                .filter(|(_, &c)| c == 4)
                .map(|(&r, _)| r)
                .max()
                .unwrap_or(0)
        }
        // 单/对/三/纯连型：首个元素即主 rank（升序）
        _ => sorted[0],
    }
}

/// 领出全量动作 + pass（响应时追加，上游 `get_legal_card_play_actions`
/// 尾部：rival 非空时追加 [] pass）。
pub(super) fn legal_responses(hand: &[i16], rival: &[i16]) -> Vec<Vec<i16>> {
    let mg = super::doudizhu_dz_move_gen::MoveGen::new(hand);
    let (rty, _rrank, rlen) = move_type(rival);
    let mut moves: Vec<Vec<i16>> = match rty {
        T_PASS => {
            // 领出：全型枚举
            mg.gen_all()
        }
        // 上游 TYPE_15_WRONG：match 落空 moves=[]，只靠尾部兜底拼装
        T_WRONG => Vec::new(),
        T_SINGLE => mg.gen_singles(),
        T_PAIR => mg.gen_pairs(),
        T_TRIPLE => mg.gen_triples(),
        T_BOMB => {
            let mut v = mg.gen_bombs();
            v.extend(mg.gen_king_bomb());
            v
        }
        T_KING_BOMB => Vec::new(),
        T_3_1 => mg.gen_3_1(),
        T_3_2 => mg.gen_3_2(),
        T_SERIAL_SINGLE => mg.gen_serial_single(rlen),
        T_SERIAL_PAIR => mg.gen_serial_pair(rlen),
        T_SERIAL_TRIPLE => mg.gen_serial_triple(rlen),
        T_SERIAL_3_1 => mg.gen_serial_3_1(rlen),
        T_SERIAL_3_2 => mg.gen_serial_3_2(rlen),
        T_4_2 => mg.gen_4_2(),
        T_4_22 => mg.gen_4_22(),
        _ => Vec::new(),
    };
    if !matches!(rty, T_PASS | T_BOMB | T_KING_BOMB) {
        moves.extend(mg.gen_bombs());
        moves.extend(mg.gen_king_bomb());
    }
    if !rival.is_empty() {
        moves.push(Vec::new()); // pass
        moves.retain(|mv| mv.is_empty() || beats_move(mv, rival));
    }
    moves
}
