//! announce 计费乘数。
//! 从 jobs.rs 按域拆出。

/// 促销强度序（只用于「多条命中时给落账留痕挑一条说得出口的代表档」，
/// **不参与倍率裁决**——见 [`billing_multipliers`] 的 P-120 说明）
pub(crate) fn promo_strength(k: &str) -> u8 {
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

/// 单档倍率表（M06 倍率表，与 api `torrents::promo` 的 UP_FACTOR/DOWN_FACTOR
/// 片段和 `factors()` 同口径——两处不一致由闸门 BA 的「角标 vs 入账」断言钉住）
fn table(k: &str) -> (f64, f64) {
    match k {
        "free" => (1.0, 0.0),
        "x2" => (2.0, 1.0),
        "x2free" => (2.0, 0.0),
        "half" => (1.0, 0.5),
        "x2half" => (2.0, 0.5),
        "p30" => (1.0, 0.3),
        _ => (1.0, 1.0),
    }
}

/// 命中促销集合的计费倍率合并：**逐维择优**（上传取最大、下载取最小）。
///
/// 审计修复（P-120，批三十七实测）：旧签名收两条 kind，先把两个维度压成
/// 一个标量强度序（p30<half<free<x2<x2half<x2free）再取「最强的一条」，
/// 于是 `free`(1,0) 输给 `x2`(2,1)——**全站 2x 活动期间，被单独标记免流的
/// 种子重新开始计下载量**（用户按 1× 下，免流被吞）；`half`(1,0.5) 遇 `x2`
/// 同样丢半价。免费与双倍是正交两维，NP/U3D 的表结构
/// （freeleech × upload_factor）也是分开的，合并必须逐维取更优的一侧。
///
/// 入参是**全部**命中的 kind（种子级 + 作用域级两条查询的结果拼起来），
/// 不再是「每级各取一条」。
pub(crate) fn billing_multipliers(kinds: &[&str]) -> (f64, f64) {
    kinds.iter().fold((1.0, 1.0), |acc, k| {
        let (up, down) = table(k);
        (acc.0.max(up), acc.1.min(down))
    })
}

/// 代表档（强度序最高的一条）：只给落账留痕用作可读出处，不参与裁决。
/// 等强时保留先出现的那条（调用方的 SQL 已按 `id DESC` 排序 ⇒ 取最新一条）。
pub(crate) fn strongest_kind<'a>(kinds: &[&'a str]) -> Option<&'a str> {
    kinds
        .iter()
        .copied()
        .fold(None, |acc: Option<&str>, k| match acc {
            Some(a) if promo_strength(a) >= promo_strength(k) => acc,
            _ => Some(k),
        })
}
