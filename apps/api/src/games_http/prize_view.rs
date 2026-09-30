//! 奖池档位的公示投影（从 overview.rs 拆出：撞 300 行上限）。
//!
//! 五个玩法（刮刮乐/九宫格/猜大小/扭蛋/大转盘）读同一张池表，投影只留一份
//! 实现 —— 两处各写一遍的话，「物品位要带图标与用途」这类补充迟早只落在一边，
//! 那就是第二份清单的开头。

use crate::games;

/// 档位投影：魔力位给倍数、物品位给图标与用途（价值口径仍走 anchor JOIN）。
pub(super) fn prize_rows(
    entries: &[games::PoolEntry],
    ticket: i64,
    icons: &[(String, String, String, Option<String>)],
    meta: &[(i16, String)],
) -> Vec<serde_json::Value> {
    entries
        .iter()
        .enumerate()
        .map(|(i, p)| {
            // payout 是既有公开字段（魔力位=倍数，物品位=0），前台一直按它渲染。
            // 倍数可以是小数（刮刮乐有 0.5x 档），一律由千分比换算。
            // 新语义一律**附加**，不替换：重命名已上线的字段等于悄悄打坏客户端。
            let mut j = serde_json::json!({
                "label": p.label,
                "weight_permille": p.weight,
                "payout": p.mult_permille() as f64 / 1000.0,
                "multiples": p.mult_permille() as f64 / 1000.0,
                "value": p.value(ticket),
                // 展示元数据：稀有度（1..5）+ 配图；纯展示，不参与 EV
                "rarity": meta.get(i).map(|m| m.0).unwrap_or(1),
                "image_url": meta
                    .get(i)
                    .map(|m| m.1.clone())
                    .unwrap_or_default(),
            });
            match &p.kind {
                games::EntryKind::Magic { .. } => {
                    j["kind"] = serde_json::json!("magic");
                }
                games::EntryKind::Item {
                    item_key,
                    qty,
                    anchor,
                } => {
                    j["kind"] = serde_json::json!("item");
                    j["item_key"] = serde_json::json!(item_key);
                    j["qty"] = serde_json::json!(qty);
                    j["anchor"] = serde_json::json!(anchor);
                    let hit = icons.iter().find(|(k, _, _, _)| k == item_key);
                    j["icon"] =
                        serde_json::json!(hit.map(|(_, v, _, _)| v.clone()));
                    j["use_kind"] =
                        serde_json::json!(hit.map(|(_, _, u, _)| u.clone()));
                    j["use_name"] = serde_json::json!(
                        hit.and_then(|(_, _, _, n)| n.clone())
                    );
                }
            }
            j
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
/// 编辑器回读这张桌时要按它复原，少了这一列就会把三区抹成一区。
pub(super) fn region_rows(
    t: &super::pool::Table,
    icons: &[(String, String, String, Option<String>)],
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (side, rows) in [("win", &t.win), ("tie", &t.tie), ("lose", &t.lose)] {
        for mut j in prize_rows(rows, t.ticket, icons, &[]) {
            j["side"] = serde_json::json!(side);
            out.push(j);
        }
    }
    out
}
