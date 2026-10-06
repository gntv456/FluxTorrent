//! 促销落账留痕（0286）。
//!
//! 单开一文件而不是改 `billing.rs`：并行会话正在改后者，同一函数两处演进必漂移。
//!
//! `traffic_ledger.promotion_kind` 自 0001 建表起就是「有列无人写」的死列
//! （SMALLINT NOT NULL DEFAULT 0），导致事后无法回答
//! 「这笔流量是按几倍率记的」——免费/双倍活动的效果核算、会员投诉仲裁、
//! 促销退款全都没有依据。这里按 `promotion_kind_enum` 的标签序写入，
//! 并把种子级与全局两条促销同时生效时的出处一并记进 reason。

/// 与 `billing::billing_multipliers` 的强度序同口径（p30 < half < free < x2 <
/// x2half < x2free）：两条同时生效时取更强的一条入库。
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

/// 返回（入库促销码, 可读出处）。两者都只描述「记这笔时生效的促销」，
/// 不参与倍率裁决——裁决仍在 `billing::billing_multipliers`。
pub(crate) fn promo_audit(
    torrent_kind: Option<&str>,
    global_kind: Option<&str>,
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
    (winner.map_or(0, code), note)
}
