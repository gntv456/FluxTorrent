//! 促销落账留痕（0286）。
//!
//! `traffic_ledger.promotion_kind` 自 0001 建表起就是「有列无人写」的死列
//! （SMALLINT NOT NULL DEFAULT 0），导致事后无法回答
//! 「这笔流量是按几倍率记的」——免费/双倍活动的效果核算、会员投诉仲裁、
//! 促销退款全都没有依据。这里按 `promotion_kind_enum` 的标签序写入，
//! 并把种子级与全局两条促销同时生效时的出处一并记进 reason。
//!
//! 批三十七（P-120）：`billing.rs` 里曾有本函数的第二份拷贝（并行会话期间
//! 各写一份，无人调用），已删——留痕口径只此一处。

/// 促销强度序：只用于「两条出处各挑一条代表档」写进可读出处，
/// 倍率与入库码都不走它（见 [`code_from_pair`]）。
fn strength(k: &str) -> u8 {
    match k {
        "p30" => 1,
        "half" => 2,
        "free" => 3,
        "x2" => 4,
        "x2half" => 5,
        "x2free" => 6,
        _ => 0,
    }
}

/// 促销码按 `promotion_kind_enum` 标签序：free=1 x2=2 x2free=3 half=4
/// x2half=5 p30=6 refundable=7；0 = 本笔无促销。
fn code(k: &str) -> i16 {
    match k {
        "free" => 1,
        "x2" => 2,
        "x2free" => 3,
        "half" => 4,
        "x2half" => 5,
        "p30" => 6,
        "refundable" => 7,
        _ => 0,
    }
}

/// 有效倍率反查档位码（P-120 配套）：`(上传倍率, 下载倍率)` 落在六档表上时
/// 直接记那一档，组合出表外档（如 x2 + p30 ⇒ 2×/0.3×）返回 None，由调用方
/// 退回「最强的一条」。
///
/// 为什么必须按有效倍率记而不是按最强档记：作弊差距判据（`jobs/audit.rs`）
/// 用 `promotion_kind NOT IN (1, 3)` 豁免免费档天然产生的 up/down 差额——
/// 若 free + 全站 x2 记成 x2(2)，免流期间照常豁免的差额会被拿去告警，
/// 做种者被误伤成 cheater。
fn code_from_pair(up: f64, down: f64) -> Option<i16> {
    match (up, down) {
        (1.0, 0.0) => Some(1), // free
        (2.0, 1.0) => Some(2), // x2
        (2.0, 0.0) => Some(3), // x2free
        (1.0, 0.5) => Some(4), // half
        (2.0, 0.5) => Some(5), // x2half
        (1.0, 0.3) => Some(6), // p30
        _ => None,
    }
}

/// 返回（入库促销码, 可读出处）。两者都只描述「记这笔时生效的促销」，
/// 不参与倍率裁决——裁决在 `billing::billing_multipliers`；
/// `effective` 是裁决后的实际倍率，促销码按它反查（见 [`code_from_pair`]）。
pub(crate) fn promo_audit(
    torrent_kind: Option<&str>,
    global_kind: Option<&str>,
    effective: (f64, f64),
) -> (i16, Option<String>) {
    let winner = match (torrent_kind, global_kind) {
        (Some(t), Some(g)) => {
            Some(if strength(t) >= strength(g) { t } else { g })
        }
        (t, g) => t.or(g),
    };
    let note = match (torrent_kind, global_kind) {
        (Some(t), Some(g)) => Some(format!("promo torrent={t} global={g}")),
        (Some(t), None) => Some(format!("promo torrent={t}")),
        (None, Some(g)) => Some(format!("promo global={g}")),
        (None, None) => None,
    };
    // 没有任何促销行时倍率只能来自券（neutral/free 券也会把下载打成 0）——
    // 那一档不在促销码里，别把券记成促销（事后按码核算促销效果会凭空多一笔）
    let code = match winner {
        None => 0,
        Some(w) => {
            code_from_pair(effective.0, effective.1).unwrap_or_else(|| code(w))
        }
    };
    (code, note)
}

#[cfg(test)]
mod tests {
    use super::promo_audit;

    /// 六档表逐档反查必须与倍率一一对应（记错一档 = 事后核算与豁免全错）
    #[test]
    fn code_follows_effective_multipliers() {
        let cases = [
            ((1.0, 0.0), 1i16),
            ((2.0, 1.0), 2),
            ((2.0, 0.0), 3),
            ((1.0, 0.5), 4),
            ((2.0, 0.5), 5),
            ((1.0, 0.3), 6),
        ];
        for (eff, want) in cases {
            assert_eq!(promo_audit(Some("x"), None, eff).0, want, "{eff:?}");
        }
    }

    /// free + x2 合成 2×/0 ⇒ 记 x2free(3)，不是「最强的一条」x2(2)
    /// ——否则作弊差距判据不豁免，免流期间的 up/down 差额全被告警（P-120）
    #[test]
    fn stacked_free_x2_is_recorded_as_x2free() {
        let (code, note) = promo_audit(Some("free"), Some("x2"), (2.0, 0.0));
        assert_eq!(code, 3, "free+x2 应记 x2free");
        assert_eq!(note.as_deref(), Some("promo torrent=free global=x2"));
    }

    /// 表外组合（x2 + p30 ⇒ 2×/0.3×）退回最强档的码，且无促销时码为 0
    #[test]
    fn unmatched_pair_falls_back_to_strongest() {
        assert_eq!(promo_audit(Some("p30"), Some("x2"), (2.0, 0.3)).0, 2);
        assert_eq!(promo_audit(None, None, (1.0, 1.0)).0, 0);
        assert_eq!(promo_audit(None, None, (1.0, 1.0)).1, None);
    }

    /// 下载被打成 0 但**没有任何促销行**（那是 free/neutral 券的效果）：
    /// 促销码必须留 0，不能把券记成促销
    #[test]
    fn voucher_only_never_fakes_a_promotion_code() {
        for eff in [(1.0, 0.0), (0.0, 0.0), (2.0, 0.0)] {
            assert_eq!(promo_audit(None, None, eff).0, 0, "{eff:?}");
        }
    }
}
