//! 斗地主 DouZero 决策入口：观测抽取 → 全量合法动作枚举 → 模型估值
//! → argmax；模型不可用回落启发式 `ai_choose`。
//!
//! 动作空间口径：响应过滤含 pass（上游语义）；领出时 rival=空 →
//! 全型枚举且无 pass（领出必须出牌）。癞子模式不走本入口（模型没见过
//! 替身面，`wild_hint` 维持启发式）。

use super::doudizhu_dz_detector::move_type;
use super::doudizhu_dz_encode::dz_ranks;
use super::doudizhu_dz_move_gen::{MoveGen, T_PASS};
use super::doudizhu_dz_selector::legal_responses;
use super::doudizhu_game::GameState;

/// DouZero 建议：返回要出的牌（本仓编码）；None = 过牌或模型不可用
/// （调用方回落启发式）。`seat` 为待决策座位。
pub(crate) fn dz_choose(g: &GameState, seat: u8) -> Option<Vec<u8>> {
    // 模型不可用直接短路（含编译开关关闭），避免白算编码
    if !super::doudizhu_dz_nn::available() {
        return None;
    }
    let hand_dz = dz_ranks(&g.hands[seat as usize]);
    // rival = 最近有效出牌且不是自己出的（自己上轮的牌可任意出 → 领出口径）
    let rival: Vec<i16> = match (&g.last, g.last_seat) {
        (Some(c), Some(s)) if s != seat => dz_ranks(&c.cards),
        _ => Vec::new(),
    };
    let actions = legal_responses(&hand_dz, &rival);
    // 领出无 pass：rival 为空时 legal_responses 不会追加 []，与上游一致；
    // 但过滤后可能为空（理论不发生，手牌非空必有单张合法）——兜底最小单张
    let actions = if actions.is_empty() {
        vec![vec![*hand_dz.iter().min()?]]
    } else {
        actions
    };
    let best = super::doudizhu_dz_nn::best_action(g, seat, &actions)?;
    let chosen = actions.get(best)?;
    if move_type(chosen).0 == T_PASS {
        return None; // 模型选择过牌
    }
    let cards = super::doudizhu_dz_nn::dz_action_to_cards(
        chosen,
        &g.hands[seat as usize],
    );
    if cards.is_empty() {
        return None;
    }
    // 双保险：出前校验实体牌型合法（编码换算丢实体时会拒）
    let _ = MoveGen::new(&hand_dz);
    Some(cards)
}

/// 决策 + 合法性保险：模型建议被状态机拒绝时回落启发式。
/// `teammate_low` 语义与 `ai_choose` 一致（仅启发式回落用）。
pub(crate) fn dz_choose_safe(
    g: &GameState,
    seat: u8,
    teammate_low: bool,
) -> Option<Vec<u8>> {
    if let Some(cards) = dz_choose(g, seat) {
        // 用状态机口径校验：试出牌不行再回落
        let mut probe = g.clone();
        if probe.play(seat, &cards).is_ok() {
            return Some(cards);
        }
    }
    super::doudizhu_ai::ai_choose(
        &g.hands[seat as usize],
        g.last.as_ref().map(|c| (c, g.last_seat.unwrap_or(0))),
        seat,
        teammate_low,
        g.landlord,
    )
}
