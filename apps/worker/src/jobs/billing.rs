//! announce 计费乘数。
//! 从 jobs.rs 按域拆出。

/// 促销强度序（0286 提为模块级：`billing_multipliers` 与 `promo_audit` 共用一份，
/// 避免「裁决口径」与「落账留痕」两处各抄一份而漂移）
fn promo_strength(k: &str) -> u8 {
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

/// 促销计费倍率（M06 倍率表，与 api domain::PromotionKind::multipliers 同口径）
pub(crate) fn billing_multipliers(
    torrent_kind: Option<&str>,
    global_kind: Option<&str>,
) -> (f64, f64) {
    let table = |k: Option<&str>| -> (f64, f64) {
        match k {
            Some("free") => (1.0, 0.0),
            Some("x2") => (2.0, 1.0),
            Some("x2free") => (2.0, 0.0),
            Some("half") => (1.0, 0.5),
            Some("x2half") => (2.0, 0.5),
            Some("p30") => (1.0, 0.3),
            _ => (1.0, 1.0),
        }
    };
    let winner = match (torrent_kind, global_kind) {
        (Some(t), Some(g)) => Some(if promo_strength(t) >= promo_strength(g) {
            t
        } else {
            g
        }),
        (t, g) => t.or(g),
    };
    table(winner)
}

/// 促销落账留痕（0286）：`traffic_ledger.promotion_kind` 自 0001 建表起
/// 就是「有列无人写」的死列，导致事后无法回答「这笔流量按几倍率记的」——
/// 免费/双倍活动的效果核算、会员投诉仲裁、促销退款都没有依据。
/// 返回值一是 `promotion_kind_enum` 的标签序（0=无促销），
/// 返回值二是可读出处（种子级与全局同时生效时两个都记）。
pub(crate) fn promo_audit(
    torrent_kind: Option<&str>,
    global_kind: Option<&str>,
) -> (i16, Option<String>) {
    let code = |k: &str| -> i16 {
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
    };
    // 与计费裁决同口径：两条同时生效时取更强的一条入库
    let winner = match (torrent_kind, global_kind) {
        (Some(t), Some(g)) => Some(if promo_strength(t) >= promo_strength(g) {
            t
        } else {
            g
        }),
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
