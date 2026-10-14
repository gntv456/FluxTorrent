//! 种子审核状态机（FRAME-PLAN-01 §7 第 6 条：判定收 domain/ 纯函数）。
//!
//! approval_status 五态：0 待审 / 1 通过 / 2 拒绝 / 3 下架 / 4 暂缓
//! （4 是 0079 批次 G7 引入的 POSTPONED 第四态）。
//!
//! 此前合法迁移判据散在四处 SQL 的 WHERE 谓词里（decide 的
//! `approval_status = 0`、batch delete 的无谓词直写 3、postpone 的
//! `IN (0, 2)`、resume 的 `= 4`），「哪些翻转合法」要靠人肉拼四段 SQL
//! 才能还原——本文件把迁移矩阵收成唯一事实源，写点改为先问
//! [`can_transition`] 再落库（行为与现状一致，见各测试注记）。
//!
//! 纯度：本层禁 actix 禁 sqlx（layer_guard 规则③），函数只做整数判定。

/// 待审
pub const PENDING: i16 = 0;
/// 已通过
pub const APPROVED: i16 = 1;
/// 已拒绝
pub const DENIED: i16 = 2;
/// 已下架（软删）
pub const DELISTED: i16 = 3;
/// 暂缓（证据不足挂起，0079 G7）
pub const POSTPONED: i16 = 4;

/// 合法迁移表：from → 允许到达的 to 集合。
///
/// 与现状（2026-10 四个写点）逐条对齐：
/// - 裁决 decide：`WHERE approval_status = 0` → 0→1、0→2；
/// - 批量下架 torrent_batch "delete"：`SET approval_status = 3` 无源态
///   谓词 → 任意现存态（含 3 自身，幂等重放下架）→3；
/// - 暂缓 postpone：`WHERE approval_status IN (0, 2)` → 0→4、2→4；
/// - 暂缓恢复 resume：`WHERE approval_status = 4` → 4→0。
const TRANSITIONS: [(i16, &[i16]); 5] = [
    (PENDING, &[APPROVED, DENIED, DELISTED, POSTPONED]),
    (APPROVED, &[DELISTED]),
    (DENIED, &[DELISTED, POSTPONED]),
    (DELISTED, &[DELISTED]),
    (POSTPONED, &[PENDING, DELISTED]),
];

/// 全部合法状态码（legal_sources 遍历用；未知码不在此列）。
const ALL_STATES: [i16; 5] = [PENDING, APPROVED, DENIED, DELISTED, POSTPONED];

/// 判定 `from → to` 是否合法。非法/未知状态码一律 false（写点据此拒）。
pub fn can_transition(from: i16, to: i16) -> bool {
    TRANSITIONS
        .iter()
        .find(|(f, _)| *f == from)
        .is_some_and(|(_, tos)| tos.contains(&to))
}

/// 允许迁入 `to` 的全部源态（SQL 侧集合过滤用：`WHERE ... = ANY($n)`）。
/// 批量下架口与 [`can_transition`] 同一事实源——矩阵改了这里跟着变，
/// 不会出现「单条判定放行、批量集合漏行」的漂移。
pub fn legal_sources(to: i16) -> Vec<i16> {
    ALL_STATES
        .iter()
        .copied()
        .filter(|&from| can_transition(from, to))
        .collect()
}

/// 允许下架（→3）的全部源态（torrent_batch "delete" 便捷封装）。
pub fn legal_delist_sources() -> Vec<i16> {
    legal_sources(DELISTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 5×5 迁移矩阵全组合断言（FRAME §7 第 6 条验收形态）。
    ///
    /// 注意两处「与直觉不同、以现状为准」的格子，均有出处：
    /// - `from == to` 除 3（下架）外全部非法：batch delete 的 UPDATE 无
    ///   源态谓词（admin_p3_http/torrent_batch.rs "delete" 动作），已下架
    ///   种重复下架是幂等重放、rows_affected 照常计数——现状放行，这里
    ///   跟随；其余态自翻无业务语义且现状没有任何写点产生，判非法；
    /// - 1→2 / 1→4 非法：已通过种没有「改判拒绝/暂缓」入口（现状如此，
    ///   要撤先下架再走重发/恢复流），3→1 的「恢复上架」同样不存在
    ///   （restore 走的是 torrent.resubmit 独立链路，不经 approval_status）。
    #[test]
    fn transition_matrix_full_5x5() {
        let expected: [[bool; 5]; 5] = [
            // to: 0      1     2     3     4
            /* from 0 */
            [false, true, true, true, true],
            /* from 1 */ [false, false, false, true, false],
            /* from 2 */ [false, false, false, true, true],
            /* from 3 */ [false, false, false, true, false],
            /* from 4 */ [true, false, false, true, false],
        ];
        for from in 0..5 {
            for to in 0..5 {
                assert_eq!(
                    can_transition(from, to),
                    expected[from as usize][to as usize],
                    "can_transition({from}, {to}) 与矩阵不符"
                );
            }
        }
    }

    /// 写点行为锚定：四个现存写点的 (from, to) 组合必须全绿，
    /// 防止矩阵表与 SQL 谓词漂移后单测仍自洽。
    #[test]
    fn write_site_predicates_stay_legal() {
        // decide：0 → 1/2
        assert!(can_transition(PENDING, APPROVED));
        assert!(can_transition(PENDING, DENIED));
        // batch delete：任意态 → 3（无源态谓词 = 全放行）
        for from in [PENDING, APPROVED, DENIED, DELISTED, POSTPONED] {
            assert!(can_transition(from, DELISTED), "delete 应允许 {from}→3");
        }
        // postpone：0/2 → 4
        assert!(can_transition(PENDING, POSTPONED));
        assert!(can_transition(DENIED, POSTPONED));
        // resume：4 → 0
        assert!(can_transition(POSTPONED, PENDING));
    }

    /// 未知状态码（负数 / 超界）不 panic、一律判非法。
    #[test]
    fn unknown_status_is_illegal() {
        assert!(!can_transition(-1, 0));
        assert!(!can_transition(0, -1));
        assert!(!can_transition(5, 0));
        assert!(!can_transition(0, 5));
        assert!(!can_transition(i16::MIN, i16::MAX));
    }

    /// legal_sources 与矩阵一致（batch delete 的 SQL 集合过滤同源）。
    #[test]
    fn legal_sources_match_matrix() {
        assert_eq!(
            legal_delist_sources(),
            vec![0, 1, 2, 3, 4],
            "现状：任意态可下架（含 3 幂等重放）"
        );
        assert_eq!(
            legal_sources(POSTPONED),
            vec![0, 2],
            "仅待审/被拒可暂缓（postpone WHERE IN (0,2)）"
        );
        assert_eq!(
            legal_sources(PENDING),
            vec![4],
            "仅暂缓可回待审（resume WHERE = 4）"
        );
        // 只有待审能迁入 1/2（裁决仅从 0 出发）；1/2 自身无入边
        assert_eq!(legal_sources(APPROVED), vec![0]);
        assert_eq!(legal_sources(DENIED), vec![0]);
    }
}
