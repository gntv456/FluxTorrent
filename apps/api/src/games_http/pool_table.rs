//! 猜大小的「一张桌」：票价 + 三个区各自的档位（从 pool.rs 拆出，撞 300 行上限）。
//!
//! 与 `load_pool` 同源同纪律：读表即关闸，配坏就拒绝服务，不猜旧值。

use sqlx::PgPool;

use crate::errors::DomainError;
use crate::games;

use super::pool::dberr;

/// 猜大小的一张桌：票价 + 三个区各自的档位。
pub(super) struct Table {
    pub ticket: i64,
    pub win: Vec<games::PoolEntry>,
    pub triple: Vec<games::PoolEntry>,
    pub lose: Vec<games::PoolEntry>,
    /// 每区展示元数据 `(rarity, image_url)`，与对应区**按位对齐**。
    /// 编辑器回读要按它复现 —— 少了它，站长设的稀有度会被下一次保存抹回默认。
    pub win_meta: Vec<(i16, String)>,
    pub triple_meta: Vec<(i16, String)>,
    pub lose_meta: Vec<(i16, String)>,
}

impl Table {
    /// 整桌档位摊平后的 EV 用同一份公式（三区合计就是一局）。
    pub fn all(&self) -> Vec<games::PoolEntry> {
        let mut v = self.win.clone();
        v.extend(self.triple.iter().cloned());
        v.extend(self.lose.iter().cloned());
        v
    }
    pub fn region(&self, side: &str) -> &[games::PoolEntry] {
        match side {
            "win" => &self.win,
            "triple" => &self.triple,
            _ => &self.lose,
        }
    }
    /// 与 `region()` 按位对齐的展示元数据
    pub fn meta(&self, side: &str) -> &[(i16, String)] {
        match side {
            "win" => &self.win_meta,
            "triple" => &self.triple_meta,
            _ => &self.lose_meta,
        }
    }
}

/// 从行表加载猜大小的桌：区由 `side` 列决定，权重仍是**一整局的千分占比**。
/// 先过 `validate_bigsmall`（机制：486/28/486）再过 `validate_pool`（EV<1），
/// 两道都在读表时跑 —— 配坏就是拒绝服务，不猜旧值。
pub(super) async fn load_table(
    db: &PgPool,
    game: &str,
) -> Result<Table, DomainError> {
    let rows: Vec<(
        i64,
        String,
        i32,
        i64,
        String,
        Option<String>,
        i32,
        Option<i64>,
        String,
        i16,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT p.ticket, e.label, e.weight, e.mult_permille,
               e.kind, e.item_key, e.qty, i.anchor, e.side,
               e.rarity, e.image_url
          FROM arcade_pools p
          JOIN arcade_pool_entries e ON e.pool_key = p.key
     LEFT JOIN arcade_items i        ON i.key = e.item_key AND i.enabled
         WHERE p.game = $1 AND p.enabled AND e.enabled
      ORDER BY p.sort, e.sort
        "#,
    )
    .bind(game)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    let ticket = rows.first().map(|r| r.0).unwrap_or(0);
    let mut t = Table {
        ticket,
        win: Vec::new(),
        triple: Vec::new(),
        lose: Vec::new(),
        win_meta: Vec::new(),
        triple_meta: Vec::new(),
        lose_meta: Vec::new(),
    };
    for (
        _,
        label,
        weight,
        mult_permille,
        kind,
        item_key,
        qty,
        anchor,
        side,
        rarity,
        image,
    ) in rows
    {
        let entry = games::PoolEntry {
            label,
            weight: u32::try_from(weight.max(0)).unwrap_or(u32::MAX),
            kind: if kind == "item" {
                games::EntryKind::Item {
                    item_key: item_key.unwrap_or_default(),
                    qty: qty.max(1),
                    anchor: anchor.unwrap_or(0),
                }
            } else {
                games::EntryKind::Magic { mult_permille }
            },
        };
        match side.as_str() {
            "win" => {
                t.win.push(entry);
                t.win_meta.push((rarity, image));
            }
            "triple" => {
                t.triple.push(entry);
                t.triple_meta.push((rarity, image));
            }
            "lose" => {
                t.lose.push(entry);
                t.lose_meta.push((rarity, image));
            }
            // any 归到输区没有意义，宁可报错：一张桌不该出现「哪侧都算」的档
            other => {
                return Err(DomainError::Validation(format!(
                "猜大小档位「{}」的 side 是「{other}」，赢/豹/输三区之外不认",
                entry.label
            )))
            }
        }
    }
    // 猜大小**机制决定只输赢魔力**：整桌不允许出现物品位。站长想给东西，走
    // 「道具加权」（game_effect）而不是把物品塞进奖池 —— 后者会让「只输赢魔力」
    // 这条口径被配置悄悄改掉，而 EV 闸看不住「物品位本身合法」这件事。
    if game == "bigsmall"
        && t.all()
            .iter()
            .any(|e| matches!(e.kind, games::EntryKind::Item { .. }))
    {
        return Err(DomainError::Validation(
            "猜大小只输赢魔力：奖池里不能有物品位（改成魔力档，或把它做成道具）"
                .into(),
        ));
    }
    games::validate_bigsmall(&t.win, &t.triple, &t.lose)
        .map_err(|e| DomainError::Validation(e.to_string()))?;
    games::validate_pool(&t.all(), ticket)
        .map_err(|e| DomainError::Validation(e.to_string()))?;
    Ok(t)
}
