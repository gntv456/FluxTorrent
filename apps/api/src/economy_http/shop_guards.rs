//! 商城发货前置（拆自 economy_http/shop.rs）。
//!
//! 拆的动机不是洁癖：`shop_buy` 顶在自己的存量行数基线上（只许瘦不许胖），
//! 而 P-348 这条「扣款之前先确认这个 kind 真发得出东西」的闸门必须落在
//! 售卖口。判定本身是纯函数，放这里既让 buy 少十几行，也能被别的售卖口
//! （后台直发、娱乐屋奖品绑定）复用同一把尺子。

use crate::errors::{DomainError, DomainResult};

/// 卡类：按张插订单行、由各自的核销口消费，不走 `apply_item_effect`。
pub(super) fn card_kind_probe(kind: &str) -> bool {
    matches!(kind, "makeup_card" | "rename_card" | "temp_invite")
}

/// 这个 SKU 品类到底发不出发放结果。
///
/// 生效链 `flux_shared::apply_item_effect` 的 match 有 `_ => {}` 兜底：
/// 不在清单里的 kind 扣了钱什么也不会发生（审计 P1 命名的「花钱买空气」）。
/// 后台建 SKU 已经过 `has_effect` 拦（admin_p3_http/shop.rs:71），但库里
/// 直接改出来的 active 行、以及「加了 match 臂却忘了加清单」的漂移都绕得过
/// 那道口 —— 所以收钱的售卖口自己也要拦一次。
pub(super) fn ensure_deliverable(kind: &str) -> DomainResult<()> {
    if crate::economy_http::has_effect(kind) || card_kind_probe(kind) {
        return Ok(());
    }
    // 整句字面量：校验详情要能按原句在 validation_details.tsv 查到；具体
    // kind 与修法只进日志（同 donate_order 的拒单口径）
    tracing::warn!(
        kind,
        "shop sale refused: item kind has no delivery path \
         (implement it in flux-shared apply_item_effect + has_effect)"
    );
    Err(DomainError::Validation(
        "该道具品类尚未接入发放逻辑，已停售".into(),
    ))
}
