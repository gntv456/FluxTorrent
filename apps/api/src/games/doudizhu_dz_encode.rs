//! 斗地主 DouZero InfoSet 编码（上游 `douzero/env/env.py` 的 `get_obs`
//! 移植）。把本仓 `GameState` 编成模型输入：`x`（373/484 维静态特征 +
//! 动作特征）与 `z`（5×162 动作历史）。
//!
//! 编码契约（与上游逐字节对齐，列序不可动）：
//! - `_cards2array`：4×13 列主序（rank 3..15 即 3..2）+ 双王 2 位 = 54；
//! - `_get_one_hot_array`：剩牌数 1-hot（17 或 20 维，0 张时下标 -1
//!   上游会崩，实际对局不会出现）；
//! - `_get_one_hot_bomb`：炸弹计数 1-hot 15 维；
//! - z：动作序列取末 15 条补零，54 维一行 reshape 5×162；
//! - x 各座位段序见 `x_landlord/x_farmer`（上游 Table 4/5）。
//!
//! 座位映射：landlord=地主座；landlord_down=地主下家；landlord_up=地主上家
//! （上游轮转 landlord→down→up）。本仓座位序 0→1→2 即同一方向。

use super::doudizhu_game::GameState;

/// x 维度（上游 Table 4/5）：landlord 373、farmer 484。
pub(crate) const X_DIM_LANDLORD: usize = 373;
pub(crate) const X_DIM_FARMER: usize = 484;

/// 本仓牌编码 → DouZero rank（3..14 普通牌、17=2、20 小王、30 大王）。
pub(super) fn dz_rank(card: u8) -> i16 {
    let r = super::doudizhu::Rank::of(card).0;
    match r {
        0..=11 => r as i16 + 3, // 3..A(14)
        12 => 17,               // 2
        13 => 20,               // 小王
        _ => 30,                // 大王
    }
}

/// 批量换 rank 并升序（DouZero 手牌口径）。
pub(super) fn dz_ranks(cards: &[u8]) -> Vec<i16> {
    let mut v: Vec<i16> = cards.iter().map(|&c| dz_rank(c)).collect();
    v.sort_unstable();
    v
}

/// DouZero rank → 54 维牌面矩阵（上游 `matrix[4,13]` flatten('F')
/// 列主序：同 rank 的 4 个计数位连续，下标 = (rank-3)*4 + i；王在 52/53）。
pub(crate) fn cards2array_ref(cards: &[i16]) -> [f32; 54] {
    cards2array(cards)
}

fn cards2array(cards: &[i16]) -> [f32; 54] {
    let mut out = [0f32; 54];
    for (r, n) in super::doudizhu_dz_move_gen::rank_counts(cards) {
        match r {
            20 => out[52] = 1.0,
            30 => out[53] = 1.0,
            3..=14 => {
                let base = ((r - 3) as usize) * 4;
                for i in 0..4 {
                    if (n as usize) > i {
                        out[base + i] = 1.0;
                    }
                }
            }
            // 上游 Card2Column：17（点数 2）独立映射 col 12（非 card-3）
            17 => {
                for i in 0..4 {
                    if (n as usize) > i {
                        out[48 + i] = 1.0;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// 剩牌数 1-hot（`max_cards` 17 或 20；n=0 时全零兜底防越界）。
fn one_hot_left(n: usize, max_cards: usize) -> Vec<f32> {
    let mut v = vec![0f32; max_cards];
    if n >= 1 && n <= max_cards {
        v[n - 1] = 1.0;
    }
    v
}

/// 炸弹数 1-hot（15 维；≥14 归 14）。
fn one_hot_bomb(n: u32) -> [f32; 15] {
    let mut v = [0f32; 15];
    let idx = (n as usize).min(14);
    v[idx] = 1.0;
    v
}

/// 动作历史取末 15 条补零后编 5×162。
fn action_seq_z(seq: &[Vec<i16>]) -> [f32; 810] {
    let mut out = [0f32; 810];
    let tail: Vec<&Vec<i16>> = seq.iter().rev().take(15).collect::<Vec<_>>();
    let padded: Vec<Vec<i16>> = (0..15 - tail.len().min(15))
        .map(|_| Vec::new())
        .chain(tail.into_iter().map(|v| v.clone()))
        .collect();
    for (i, chunk) in padded.chunks(3).enumerate() {
        for (j, mv) in chunk.iter().enumerate() {
            let arr = cards2array(mv);
            // 三个 54 拼成一行 162 → 放到 (i, j*54..j*54+54)
            for (k, &v) in arr.iter().enumerate() {
                out[i * 162 + j * 54 + k] = v;
            }
        }
    }
    out
}

/// 座位 → 上游位置序（landlord=0, landlord_down=1, landlord_up=2 的
/// 本仓座位号）。down = 地主下家（轮转下一座）。
fn pos_seats(landlord: u8) -> [u8; 3] {
    [landlord, (landlord + 1) % 3, (landlord + 2) % 3]
}

/// 从 GameState 抽出编码所需的全部原始量。
pub(super) struct DzObs {
    /// 我的手牌（DouZero rank 升序）。
    pub hand: Vec<i16>,
    /// 两家手牌并集。
    pub other: Vec<i16>,
    /// 最近有效出牌（过牌穿透后）。
    pub last_move: Vec<i16>,
    /// 各位置最近一次动作（landlord/down/up）。
    pub last_move_dict: [Vec<i16>; 3],
    /// 各位置已出牌并集。
    pub played: [Vec<i16>; 3],
    /// 各位置剩牌数（landlord/down/up）。
    pub left: [usize; 3],
    /// 炸弹计数。
    pub bombs: u32,
    /// 全局动作序列（含 pass 空手）。
    pub seq: Vec<Vec<i16>>,
}

/// 从状态与视角座位抽观测（`seat` 是要出牌的座）。
pub(super) fn extract(g: &GameState, seat: u8) -> DzObs {
    let landlord = g.landlord.unwrap_or(0);
    let seats = pos_seats(landlord);
    // 动作序列：events 里 play/pass（bid/grab/double/discard 不入序列——
    // 上游无这些阶段；地主弃三张也不算出牌）
    let seq: Vec<Vec<i16>> = g
        .events
        .iter()
        .filter(|e| e.action == "play" || e.action == "pass")
        .map(|e| dz_ranks(&e.cards))
        .collect();
    // 最近有效出牌：序列末尾穿透 pass
    let last_move = seq
        .iter()
        .rev()
        .find(|mv| !mv.is_empty())
        .cloned()
        .unwrap_or_default();
    // 各位置最近动作：倒序找该座最近的 play/pass
    let mut last_dict: [Vec<i16>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    let play_events: Vec<(u8, Vec<i16>)> = g
        .events
        .iter()
        .filter(|e| e.action == "play" || e.action == "pass")
        .map(|e| (e.seat, dz_ranks(&e.cards)))
        .collect();
    for (pos, &s) in seats.iter().enumerate() {
        if let Some((_, mv)) = play_events.iter().rev().find(|(es, _)| *es == s)
        {
            last_dict[pos] = mv.clone();
        }
    }
    // 各位置已出牌并集（play 事件的牌；不含弃三张与底牌）
    let mut played: [Vec<i16>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (pos, &s) in seats.iter().enumerate() {
        played[pos] = play_events
            .iter()
            .filter(|(es, _)| *es == s)
            .flat_map(|(_, mv)| mv.iter().copied())
            .collect();
    }
    let left = [
        g.hands[seats[0] as usize].len(),
        g.hands[seats[1] as usize].len(),
        g.hands[seats[2] as usize].len(),
    ];
    let other: Vec<i16> = (0..3usize)
        .filter(|&i| seats[i] != seat)
        .flat_map(|i| dz_ranks(&g.hands[seats[i] as usize]))
        .collect();
    let my_pos = seats.iter().position(|&s| s == seat).unwrap_or(0);
    DzObs {
        hand: dz_ranks(&g.hands[seat as usize]),
        other,
        last_move,
        last_move_dict: last_dict,
        played,
        left,
        bombs: g.bombs,
        seq,
    }
}

/// x 的静态段（不含动作特征），按视角拼上游段序。
/// landlord 视角 373-54=319 维；farmer 视角 484-54=430 维。
fn x_static(o: &DzObs, my_pos: usize) -> Vec<f32> {
    let mut x = Vec::with_capacity(430);
    let hand = cards2array(&o.hand);
    let other = cards2array(&o.other);
    let last = cards2array(&o.last_move);
    x.extend(hand);
    x.extend(other);
    if my_pos == 0 {
        // landlord：last, up_played, down_played, up_left, down_left, bombs
        x.extend(last);
        x.extend(cards2array(&o.played[2]));
        x.extend(cards2array(&o.played[1]));
        x.extend(one_hot_left(o.left[2], 17));
        x.extend(one_hot_left(o.left[1], 17));
    } else {
        // down(1)/up(2)：landlord_played, teammate_played, last_action,
        // last_landlord, last_teammate, landlord_left, teammate_left, bombs
        let mate = if my_pos == 1 { 2 } else { 1 }; // 上游队友定义
        x.extend(cards2array(&o.played[0]));
        x.extend(cards2array(&o.played[mate]));
        x.extend(last);
        x.extend(cards2array(&o.last_move_dict[0]));
        x.extend(cards2array(&o.last_move_dict[mate]));
        x.extend(one_hot_left(o.left[0], 20));
        x.extend(one_hot_left(o.left[mate], 17));
    }
    x.extend(one_hot_bomb(o.bombs));
    x
}

/// 编码：返回 (x_batch[N×dim], z_batch[N×5×162])，N = 动作数。
/// `actions` 是 DouZero rank 列表的候选集（含 pass 空手）。
pub(super) fn encode(
    g: &GameState,
    seat: u8,
    actions: &[Vec<i16>],
) -> Option<(Vec<f32>, Vec<f32>, usize)> {
    let o = extract(g, seat);
    let landlord = g.landlord.unwrap_or(0);
    let seats = pos_seats(landlord);
    let my_pos = seats.iter().position(|&s| s == seat)?;
    let dim = if my_pos == 0 {
        X_DIM_LANDLORD
    } else {
        X_DIM_FARMER
    };
    let static_x = x_static(&o, my_pos);
    let z = action_seq_z(&o.seq);
    let n = actions.len();
    if n == 0 {
        return None;
    }
    let mut x = Vec::with_capacity(n * dim);
    for a in actions {
        x.extend_from_slice(&static_x);
        x.extend_from_slice(&cards2array(a));
    }
    let mut z_all = Vec::with_capacity(n * 810);
    for _ in 0..n {
        z_all.extend_from_slice(&z);
    }
    Some((x, z_all, dim))
}
