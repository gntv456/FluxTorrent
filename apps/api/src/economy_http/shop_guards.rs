//! 商城发货前置（拆自 economy_http/shop.rs）。
//!
//! 拆的动机不是洁癖：`shop_buy` 顶在自己的存量行数基线上（只许瘦不许胖），
//! 而 P-348 这条「扣款之前先确认这个 kind 真发得出东西」的闸门必须落在
//! 售卖口。判定本身是纯函数，放这里既让 buy 少十几行，也能被别的售卖口
//! （后台直发、娱乐屋奖品绑定）复用同一把尺子。

use crate::errors::{DomainError, DomainResult};

/// 卡类：按张插订单行、由各自的核销口消费，不走 `apply_item_effect`。
pub(super) fn card_kind_probe(kind: &str) -> bool {
    matches!(kind, "makeup_card" | "temp_invite")
}

/// 「花得掉」清单转发到共享层（`flux_shared::redeem_port_exists`，P-477）：
/// 发卡/发券那一侧在本 crate 的 `apply_item_effect`，而问这把尺子的有两个口
/// ——商城售卖口与娱乐屋奖品绑定口。清单写在这里就会出现「售卖停了、奖品照发」
/// 的半个闸门，所以只留一份。
pub(super) fn redeem_port_exists(kind: &str) -> bool {
    flux_shared::redeem_port_exists(kind)
}

/// 统一的拒单出口：整句字面量（校验详情要能按原句在 validation_details.tsv
/// 查到）；具体 kind 与修法只进日志（同 donate_order 的拒单口径）。
fn refuse(kind: &str) -> DomainResult<()> {
    tracing::warn!(
        kind,
        "shop sale refused: item kind has no delivery path \
         (implement it in flux-shared apply_item_effect + has_effect, \
         and a redeem port for card/voucher kinds)"
    );
    Err(DomainError::Validation(
        "该道具品类尚未接入发放逻辑，已停售".into(),
    ))
}

/// 这个 SKU 品类到底发不出发放结果、发出来的东西花不花得掉。
///
/// 生效链 `flux_shared::apply_item_effect` 的 match 有 `_ => {}` 兜底：
/// 不在清单里的 kind 扣了钱什么也不会发生（审计 P1 命名的「花钱买空气」）。
/// 后台建 SKU 已经过 `has_effect` 拦（admin_p3_http/shop.rs:71），但库里
/// 直接改出来的 active 行、以及「加了 match 臂却忘了加清单」的漂移都绕得过
/// 那道口 —— 所以收钱的售卖口自己也要拦一次。
pub(super) fn ensure_deliverable(kind: &str) -> DomainResult<()> {
    // P-477：先发券/发卡、后面没人认 ⇒ 别收钱。成对判据在共享层，
    // 娱乐屋奖品绑定口（games_http/item_use.rs）问的是同一句。
    if flux_shared::voucher_without_redeem_port(kind) {
        return refuse(kind);
    }
    if crate::economy_http::has_effect(kind) || card_kind_probe(kind) {
        return Ok(());
    }
    refuse(kind)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(kind: &str) -> bool {
        match ensure_deliverable(kind) {
            Ok(()) => false,
            Err(DomainError::Validation(m)) => m.contains("尚未接入发放逻辑"),
            Err(_) => false,
        }
    }

    #[test]
    fn deliverable_and_redeemable_kinds_sell() {
        // 有发放臂的状态类
        assert!(!refused("upload_credit"));
        assert!(!refused("vip"));
        assert!(!refused("charity"));
        // 卡/券类：发得出且核销口在册
        assert!(!refused("makeup_card"));
        assert!(!refused("temp_invite"));
        assert!(!refused("voucher_free"));
        assert!(!refused("voucher_neutral"));
    }

    #[test]
    fn voucher_with_no_redeem_port_is_refused() {
        // P-477 现场：改名卡发得出券，但全仓没有核销口 ⇒ 售卖口拦在扣款前
        assert!(refused("rename_card"));
        // 白名单与核销口清单不许再漂移：card_kind_probe 里的每一档都必须花得掉
        for k in ["makeup_card", "temp_invite"] {
            assert!(card_kind_probe(k) && redeem_port_exists(k), "{k}");
        }
        // 未知品类同样拒（老行为不回归）
        assert!(refused("no_such_kind"));
    }
}
