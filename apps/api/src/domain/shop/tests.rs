//! 商城履约状态机的单测（最坏后果：重复发货 / 永久吞钱）。

use super::*;

fn inp(state: State, ok: bool, secs: i64, retry: bool) -> Input {
    Input {
        state,
        amount: 100,
        fulfill_ok: ok,
        frozen_secs: secs,
        grace_secs: 120,
        is_retry: retry,
    }
}

/// 状态解析：未知值按 Done（绝不当未完成去重试）。
#[test]
fn parse_unknown_is_done_not_retry() {
    assert_eq!(State::parse("frozen"), State::Frozen);
    assert_eq!(State::parse("garbage"), State::Done);
    assert_eq!(State::parse(""), State::Done);
}

/// 占位中 → 落冻结。
#[test]
fn reserving_freezes() {
    assert_eq!(
        advance(&inp(State::Reserving, false, 0, false)),
        Action::Freeze { amount: 100 }
    );
}

/// 已冻结 + 履约成功 → 扣款成交。
#[test]
fn frozen_ok_fulfills() {
    assert_eq!(
        advance(&inp(State::Frozen, true, 10, false)),
        Action::Fulfill
    );
}

/// 已冻结 + 履约失败 + 仍在宽限 → 不动（等下一轮）。
#[test]
fn frozen_fail_within_grace_waits() {
    assert_eq!(advance(&inp(State::Frozen, false, 10, false)), Action::None);
}

/// 已冻结 + 履约失败 + 宽限已过 → 解冻退款（钱不能永久卡住）。
#[test]
fn frozen_fail_after_grace_releases() {
    assert_eq!(
        advance(&inp(State::Frozen, false, 200, false)),
        Action::Release {
            amount: 100,
            reason: "fulfill_failed".into(),
        }
    );
}

/// 已冻结 + 恢复任务重试 + 仍失败 → 立刻退款（不无限重试）。
#[test]
fn retry_failure_releases_immediately() {
    assert_eq!(
        advance(&inp(State::Frozen, false, 5, true)),
        Action::Release {
            amount: 100,
            reason: "fulfill_failed".into(),
        }
    );
}

/// Failed 态重试成功 → 履约（把之前的失败补救回来）。
#[test]
fn failed_retry_success_fulfills() {
    assert_eq!(
        advance(&inp(State::Failed, true, 999, true)),
        Action::Fulfill
    );
}

/// Failed 态重试仍失败 → 退款（单次重试是刻意的）。
#[test]
fn failed_retry_failure_releases() {
    let a = advance(&inp(State::Failed, false, 999, true));
    assert!(matches!(a, Action::Release { .. }));
    assert_eq!(
        a,
        Action::Release {
            amount: 100,
            reason: "fulfill_failed_after_retry".into(),
        }
    );
}

/// 终态绝不动作——重复发货比状态不漂亮严重得多。
#[test]
fn terminal_states_are_noop() {
    for st in [State::Done, State::Refunded] {
        assert_eq!(advance(&inp(st, true, 999, true)), Action::None);
        assert_eq!(advance(&inp(st, false, 0, false)), Action::None);
        assert!(st.is_terminal());
    }
}

/// 递增价格：未过 anchor 不涨。
#[test]
fn price_flat_before_anchor() {
    assert_eq!(
        price_with_step(100, 3, Some(10), Some(500), 10).unwrap(),
        100
    );
    assert_eq!(
        price_with_step(100, 10, Some(10), Some(500), 10).unwrap(),
        100,
        "正好等于 anchor 仍不涨"
    );
}

/// 递增价格：过 anchor 每份涨 step。
#[test]
fn price_steps_after_anchor() {
    // anchor=10, sold=12 -> 涨 2 份 * 10 = 120
    assert_eq!(
        price_with_step(100, 12, Some(10), Some(500), 10).unwrap(),
        120
    );
}

/// 递增价格：封顶。
#[test]
fn price_capped_at_max() {
    assert_eq!(
        price_with_step(100, 1000, Some(10), Some(150), 10).unwrap(),
        150
    );
}

/// 无 step → 原价（绝大多数商品）。
#[test]
fn no_step_means_flat_price() {
    assert_eq!(price_with_step(100, 999, None, Some(500), 0).unwrap(), 100);
}

/// 缺 max 时直接报错——配置漏填要让站长看见，不该偷偷封顶。
#[test]
fn missing_max_is_rejected_not_silently_capped() {
    // 偷偷兜底会得到「明明配了递增却卖不动」这种查不出原因的现场
    assert!(price_with_step(100, 1_000_000, Some(10), None, 10).is_err());
    assert!(
        price_with_step(100, 1, Some(0), None, 0).is_err(),
        "step=0 也不该放行——配了递增就说明打算涨，得配上限"
    );
}

/// 负数输入必须拒。
#[test]
fn negative_inputs_rejected() {
    assert!(price_with_step(-1, 0, None, None, 0).is_err());
    assert!(price_with_step(100, -1, None, None, 0).is_err());
}

/// step 配置校验：启用递增必须设上限。
#[test]
fn validate_step_requires_max() {
    assert!(validate_step(None, None, 0).is_ok());
    assert!(validate_step(Some(0), None, 0).is_ok(), "step=0 等于不涨");
    assert!(validate_step(Some(10), None, 0).is_err(), "涨但没上限");
    assert!(validate_step(Some(10), Some(500), 0).is_ok());
    assert!(validate_step(Some(-1), Some(500), 0).is_err());
    assert!(validate_step(Some(10), Some(-1), 0).is_err());
    assert!(validate_step(Some(10), Some(500), -1).is_err());
}
