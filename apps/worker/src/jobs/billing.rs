//! announce 计费乘数。
//! 从 jobs.rs 按域拆出。

/// 促销计费倍率（M06 倍率表，与 api domain::PromotionKind::multipliers 同口径）
pub(crate) fn billing_multipliers(
    torrent_kind: Option<&str>,
    global_kind: Option<&str>,
) -> (f64, f64) {
    let strength = |k: &str| -> u8 {
        match k {
            "p30" => 1,
            "half" => 2,
            "free" => 3,
            "x2" => 4,
            "x2half" => 5,
            "x2free" => 6,
            _ => 0,
        }
    };
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
        (Some(t), Some(g)) => {
            Some(if strength(t) >= strength(g) { t } else { g })
        }
        (t, g) => t.or(g),
    };
    table(winner)
}
