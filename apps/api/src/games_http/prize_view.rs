//! 奖池档位的公示投影（从 overview.rs 拆出：撞 300 行上限）。
//!
//! 五个玩法（刮刮乐/九宫格/猜大小/扭蛋/大转盘）读同一张池表，投影只留一份
//! 实现 —— 两处各写一遍的话，「物品位要带图标与用途」这类补充迟早只落在一边，
//! 那就是第二份清单的开头。

use crate::games;

/// 一档奖品的公开投影（`/games` 各池的 `prizes` 行）。
///
/// 用**具名结构体**而不是 `json!` 字面量：契约门禁（`scripts/check_type_drift.mjs`）
/// 只比对具名结构体 —— 动态拼出来的形状是它的盲区，而那正是
/// 「TS 声明了、Rust 没返回 → 前端读到 undefined → 页面静默空白」的温床。
/// 改了这里的字段，记得同步 `apps/web/lib/games.ts` 的 `JggPrizeView`
/// （门禁表里已挂这一对，漏改会当场 FAIL）。
#[derive(serde::Serialize)]
pub(super) struct PrizeRow {
    pub label: String,
    pub weight_permille: u32,
    /// 魔力位=票价倍数（0.5x 这类小数也走这里）；物品位恒 0
    pub payout: f64,
    /// 与 payout 同值：历史字段名，前端两侧都在读
    pub multiples: f64,
    /// 该档魔力等值（物品位 = anchor × 件数）
    pub value: i64,
    /// 展示元数据（不参与 EV）：稀有度 1..5
    pub rarity: i16,
    /// 展示元数据（不参与 EV）：站长配的档位配图 URL
    pub image_url: String,
    pub kind: String,
    /// 以下仅物品位出现
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qty: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// 用途（迁移 0246）：collect | spark | sku
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_name: Option<String>,
    /// 猜大小专用：这一档在哪一区付（win | tie | lose）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
}

/// 档位投影：魔力位给倍数、物品位给图标与用途（价值口径仍走 anchor JOIN）。
pub(super) fn prize_rows(
    entries: &[games::PoolEntry],
    ticket: i64,
    icons: &[(String, String, String, Option<String>)],
    meta: &[(i16, String)],
) -> Vec<PrizeRow> {
    entries
        .iter()
        .enumerate()
        .map(|(i, p)| {
            // payout/multiples 是既有公开字段（魔力位=倍数，物品位=0），
            // 前台两侧都在读，所以两个都发；新语义一律**附加**，不替换。
            let mult = p.mult_permille() as f64 / 1000.0;
            let mut row = PrizeRow {
                label: p.label.clone(),
                weight_permille: p.weight,
                payout: mult,
                multiples: mult,
                value: p.value(ticket),
                rarity: meta.get(i).map(|m| m.0).unwrap_or(1),
                image_url: meta.get(i).map(|m| m.1.clone()).unwrap_or_default(),
                kind: "magic".to_string(),
                item_key: None,
                qty: None,
                anchor: None,
                icon: None,
                use_kind: None,
                use_name: None,
                side: None,
            };
            if let games::EntryKind::Item {
                item_key,
                qty,
                anchor,
            } = &p.kind
            {
                row.kind = "item".to_string();
                row.item_key = Some(item_key.clone());
                row.qty = Some(*qty);
                row.anchor = Some(*anchor);
                let hit = icons.iter().find(|(k, _, _, _)| k == item_key);
                row.icon = hit.map(|(_, v, _, _)| v.clone());
                row.use_kind = hit.map(|(_, _, u, _)| u.clone());
                row.use_name = hit.and_then(|(_, _, _, n)| n.clone());
            }
            row
        })
        .collect()
}

/// 不中奖档（倍率 0）的合计概率。前台「谢谢参与」那一行读它。
pub(super) fn scratch_empty_pct(entries: &[games::PoolEntry]) -> f64 {
    let total: u64 = entries.iter().map(|e| u64::from(e.weight)).sum();
    entries
        .iter()
        .filter(|e| e.mult_permille() == 0)
        .map(|e| pct_of(e.weight, total))
        .sum()
}

fn pct_of(weight: u32, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        f64::from(weight) * 100.0 / total as f64
    }
}

/// 猜大小的公示行：与 prize_rows 同一份形状，只是每行多带「在哪一区付」。
/// 编辑器回读这张桌时要按它复原，少了这一列就会把三区抹成一区；
/// 同理，展示元数据（稀有度/配图）也要按区带出来，否则站长设的稀有度
/// 在回读时恒为默认 1，下一次保存就把真值抹掉了。
pub(super) fn region_rows(
    t: &super::pool::Table,
    icons: &[(String, String, String, Option<String>)],
) -> Vec<PrizeRow> {
    let mut out = Vec::new();
    for side in ["win", "tie", "lose"] {
        let (rows, meta) = (t.region(side), t.meta(side));
        for mut row in prize_rows(rows, t.ticket, icons, meta) {
            row.side = Some(side.to_string());
            out.push(row);
        }
    }
    out
}
