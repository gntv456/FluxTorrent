//! 农场「收获彩蛋」：确定性收获之上的一档随机奖励，读 `arcade_pools` 行表。
//!
//! 农场与另三个玩法不同 —— 它本身不是抽奖（收获量由作物表 × 市场窗口决定），
//! 所以这里加的不是收获本身，而是站长想加才加的**彩蛋**：另外按权重决定
//! 要不要多给一点魔力或一件目录里的物品。
//!
//! 经济口径：基础回收率是机制常数 `games::FARM_BASE_EV = 0.90`，彩蛋只能吃
//! 剩下的 0.10。定标单位取**活的** `min(farm_crops.seed_price)`（见 `load_farm`），
//! 配超了写侧拒收、运行时同样拒收（`games::validate_farm`）。

use sqlx::PgPool;

use crate::errors::DomainError;
use crate::games;

use super::pool::{dberr, grant_item, GrantOutcome};

/// 农场彩蛋池：定标单位（最便宜种子价）+ 档位。
pub(super) struct FarmPool {
    pub unit: i64,
    pub entries: Vec<games::PoolEntry>,
}

/// 彩蛋池的定标单位 = **现役最便宜种子价**（下架的不算，下架等于不再供应种子）。
/// 写侧闸、作物保存的跨表回查与运行时都读这同一个查询；
/// 两处各查一遍的话，「新加一株便宜作物」就会只被一侧看见。
pub(super) async fn farm_unit<'e, E>(exe: E) -> Result<i64, DomainError>
where
    E: sqlx::PgExecutor<'e>,
{
    let unit: Option<i64> = sqlx::query_scalar(
        "SELECT min(seed_price)::bigint FROM farm_crops WHERE active",
    )
    .fetch_optional(exe)
    .await
    .map_err(dberr)?;
    Ok(unit.unwrap_or(0))
}

/// 农场彩蛋池的档位（事务内也读得到未提交的改动 —— 作物保存要的就是这个）。
pub(super) async fn farm_entries<'e, E>(
    exe: E,
) -> Result<Vec<games::PoolEntry>, DomainError>
where
    E: sqlx::PgExecutor<'e>,
{
    let rows: Vec<(
        String,
        i32,
        i64,
        String,
        Option<String>,
        i32,
        Option<i64>,
    )> = sqlx::query_as(
        r#"
            SELECT e.label, e.weight, e.mult_permille,
                   e.kind, e.item_key, e.qty, i.anchor
              FROM arcade_pools p
              JOIN arcade_pool_entries e ON e.pool_key = p.key
         LEFT JOIN arcade_items i        ON i.key = e.item_key AND i.enabled
             WHERE p.game = 'farm' AND p.enabled AND e.enabled
          ORDER BY e.sort
        "#,
    )
    .fetch_all(exe)
    .await
    .map_err(dberr)?;
    Ok(rows
        .into_iter()
        .map(
            |(label, weight, mult_permille, kind, item_key, qty, anchor)| {
                games::PoolEntry {
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
                }
            },
        )
        .collect())
}

/// 读农场彩蛋池并**关闸校验**。单位用活的 `min(seed_price)` 而不是池上记的
/// ticket：站长新加（或改便宜）一株作物，就会让同一档彩蛋的相对价值变大，
/// 用旧 ticket 算等于给增发留门。
pub(super) async fn load_farm(db: &PgPool) -> Result<FarmPool, DomainError> {
    let unit = farm_unit(db).await?;
    let entries = farm_entries(db).await?;
    // 没配池子等于「什么都不加」：这是合法状态，不该因此把收获打死
    if !entries.is_empty() {
        games::validate_farm(&entries, unit)
            .map_err(|e| DomainError::Validation(e.to_string()))?;
    }
    Ok(FarmPool { unit, entries })
}

/// 抽中的那一档彩蛋。`extra` 已按这一株的种子价折算好。
pub(super) struct Egg {
    pub label: String,
    pub extra: i64,
    /// 物品彩蛋：(目录 key, 件数, 单件权威折算价)
    item: Option<(String, i32, i64)>,
}

/// 收获前抽一次彩蛋。没配池 / 池里只有一档空档时 `extra=0`，不发物品。
pub(super) async fn roll_egg(
    db: &PgPool,
    seed_price: i64,
) -> Result<Option<Egg>, DomainError> {
    let pool = load_farm(db).await?;
    let draw = games::draw_entry(&pool.entries);
    let Some(d) = draw else { return Ok(None) };
    let (extra, item) = match &d.prize.kind {
        games::EntryKind::Magic { mult_permille } => {
            (games::egg_pay(seed_price, *mult_permille), None)
        }
        games::EntryKind::Item {
            item_key,
            qty,
            anchor,
        } => (0, Some((item_key.clone(), *qty, *anchor))),
    };
    Ok(Some(Egg {
        label: d.prize.label,
        extra,
        item,
    }))
}

impl Egg {
    /// 收获账落定之后发这一档：物品走发放账（自带幂等与库存判定），
    /// 魔力已经在同一笔 `earn_spark_tx` 里入账，这里只回投影。
    pub(super) async fn settle(
        self,
        db: &PgPool,
        user_id: i64,
        plot_id: i64,
    ) -> Result<serde_json::Value, DomainError> {
        let Some((key, qty, anchor)) = self.item else {
            return Ok(serde_json::json!({
                "label": self.label, "kind": "magic", "extra": self.extra,
            }));
        };
        let idem = format!("farm-harvest-item:{plot_id}");
        let outcome =
            grant_item(db, user_id, &key, qty, "farm_harvest", "rand", &idem)
                .await?;
        Ok(match outcome {
            GrantOutcome::Granted => serde_json::json!({
                "label": self.label, "kind": "item", "item_key": key,
                "qty": qty, "value": anchor * i64::from(qty),
            }),
            GrantOutcome::FellBack(why) => serde_json::json!({
                "label": self.label, "kind": "fallback", "fell_back": why,
            }),
        })
    }
}
