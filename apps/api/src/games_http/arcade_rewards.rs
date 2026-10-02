//! 确定侧奖励（周常任务 / 赛季里程碑）：读 0247 的行表，并统一「领取时怎么发」。
//!
//! 为什么确定侧要单独一套口径：随机侧有 EV 闸（奖池综合返还必须 <1），
//! 而确定侧是「达到条件就必给」，EV 闸**结构上管不到它**。所以这一侧的每一笔
//! 发放都必须被发放预算看见：魔力按 `spark_ledger(kind='arcade')` 计，
//! 物品按 `arcade_items.anchor × 件数` 折算计（见 `det_cost_value`）。
//!
//! 物品发不出去时（停用 / 全服库存耗尽 / 每人上限）确定侧**按 anchor 全额折魔力**，
//! 不像随机侧那样按 FALLBACK_MULT 打折 —— 随机侧回落是「没抽中实物」的补偿价，
//! 而这里是站长许下的确定承诺，价值一分不能少。

use sqlx::PgPool;

use super::pool::{dberr, grant_item, GrantOutcome};
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};

/// 一行奖励：`target` 对周常是局数、对里程碑是票根数
#[derive(Debug, Clone)]
pub(super) struct Reward {
    pub code: String,
    pub game_ref: String,
    pub target: i64,
    pub reward_spark: i64,
    pub item_key: Option<String>,
    pub item_qty: i32,
    /// 物品目录里的名字（前台与公示要显示「+1 件 什么」）
    pub item_name: Option<String>,
}

/// 当前赛季 key：设置键可配，缺省 S1（换赛季是运营动作，不该改代码重发）
pub(super) async fn season_key(db: &PgPool) -> DomainResult<String> {
    let k: String = sqlx::query_scalar(
        "SELECT COALESCE(\
           (SELECT value FROM site_settings \
             WHERE name = 'arcade_season_key'), 'S1')",
    )
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(k)
}

fn map_row(
    code: String,
    game_ref: String,
    target: i32,
    reward_spark: i64,
    item_key: Option<String>,
    item_qty: i32,
    item_name: Option<String>,
) -> Reward {
    Reward {
        code,
        game_ref,
        target: i64::from(target),
        reward_spark,
        item_key,
        item_qty,
        item_name,
    }
}

/// 两套奖励行的读取形状相同（只是列名不同），别名一下免得写两遍长元组
type RewardRows = Vec<(
    String,
    String,
    i32,
    i64,
    Option<String>,
    i32,
    Option<String>,
)>;

/// 周常（启用中的，按 sort）
pub(super) async fn load_quests(db: &PgPool) -> DomainResult<Vec<Reward>> {
    let rows: RewardRows = sqlx::query_as(
        "SELECT q.code, q.game_ref, q.target, q.reward_spark, \
                    q.item_key, q.item_qty, i.name \
               FROM arcade_quests q \
          LEFT JOIN arcade_items i ON i.key = q.item_key \
              WHERE q.enabled ORDER BY q.sort, q.code",
    )
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    Ok(rows
        .into_iter()
        .map(|(c, g, t, r, ik, q, nm)| map_row(c, g, t, r, ik, q, nm))
        .collect())
}

/// 当前赛季的里程碑。归属玩法固定为票根册，`game_ref` 用字面量占位，
/// 让两套奖励在前台与领取里走同一个形状（少一处分支就少一处漂移）。
pub(super) async fn load_milestones(
    db: &PgPool,
    season: &str,
) -> DomainResult<Vec<Reward>> {
    let rows: RewardRows = sqlx::query_as(
        "SELECT m.code, 'stub', m.need, m.reward_spark, \
                    m.item_key, m.item_qty, i.name \
               FROM arcade_milestones m \
          LEFT JOIN arcade_items i ON i.key = m.item_key \
              WHERE m.enabled AND m.season_key = $1 \
           ORDER BY m.sort, m.code",
    )
    .bind(season)
    .fetch_all(db)
    .await
    .map_err(dberr)?;
    Ok(rows
        .into_iter()
        .map(|(c, g, t, r, ik, q, nm)| map_row(c, g, t, r, ik, q, nm))
        .collect())
}

/// 按 anchor 反查一件物品的折算价（回落定价用）。
/// Executor 版：池连接与事务连接都能喂（award_tx 里要在同一事务读）。
async fn anchor_of(
    db: impl sqlx::Executor<'_, Database = sqlx::Postgres>,
    key: &str,
) -> DomainResult<i64> {
    let a: Option<i64> = sqlx::query_scalar(
        "SELECT anchor FROM arcade_items WHERE key = $1 AND enabled",
    )
    .bind(key)
    .fetch_optional(db)
    .await
    .map_err(dberr)?;
    Ok(a.unwrap_or(0))
}

/// 发放一行奖励：魔力 + 物品，全部幂等。返回 (入账魔力, 物品说明, 回落原因)
///
/// 事务版（2026-10 审计 P2）：claim 落库、物品发放、魔力入账并进**同一笔**。
/// 旧实现三段各自提交，claim 落库后任一步失败该期奖励永久丢失
/// （重试报「已领取」）。fallback_to_db 仅供无法开外层事务的旧调用。
pub(super) async fn award_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    r: &Reward,
    idem: &str,
) -> DomainResult<(i64, Option<String>, Option<String>)> {
    let mut spark = r.reward_spark;
    let mut got_item: Option<String> = None;
    let mut fell: Option<String> = None;
    if let Some(key) = &r.item_key {
        let qty = r.item_qty.max(1);
        let game = format!("arcade:{}", r.code);
        let gidem = format!("{idem}:item");
        match super::pool::grant_item_tx(
            tx, uid, key, qty, &game, "det", &gidem,
        )
        .await?
        {
            GrantOutcome::Granted => got_item = Some(key.clone()),
            GrantOutcome::FellBack(why) => {
                let anchor = anchor_of(&mut **tx, key).await?;
                if anchor <= 0 {
                    // 报不出价值就不能默默吞掉这份奖励
                    return Err(DomainError::Validation(format!(
                        "奖励「{}」引用的物品「{key}」已停用或没有折算价，\
                         无法回落成魔力：先在物品目录里修好它再来领取",
                        r.code
                    )));
                }
                spark += anchor * i64::from(qty);
                fell = Some(why.to_string());
            }
        }
    }
    if spark > 0 {
        crate::economy_http::earn_spark_tx(
            tx,
            uid,
            spark,
            "arcade",
            &format!("{idem}:spark"),
        )
        .await?;
    }
    Ok((spark, got_item, fell))
}

/// 独立事务包装（无外层事务时的便捷入口；原子性弱于 award_tx）
pub(super) async fn award(
    db: &PgPool,
    uid: i64,
    r: &Reward,
    idem: &str,
) -> DomainResult<(i64, Option<String>, Option<String>)> {
    let mut tx = db.begin().await.map_err(dberr)?;
    let out = award_tx(&mut tx, uid, r, idem).await?;
    tx.commit().await.map_err(dberr)?;
    Ok(out)
}

/// 后台编辑器要看到**全部**行（含停用的），否则停用一条就再也编辑不回来。
/// 玩法侧读的仍是 `load_*`（只给启用中的），两边同表同列，只差一个 WHERE。
pub(super) async fn admin_rows(db: &PgPool) -> DomainResult<serde_json::Value> {
    let quests: Vec<(String, String, i32, i64, Option<String>, i32, bool)> =
        sqlx::query_as(
            "SELECT code, game_ref, target, reward_spark, item_key, \
                    item_qty, enabled \
               FROM arcade_quests ORDER BY sort, code",
        )
        .fetch_all(db)
        .await
        .map_err(dberr)?;
    let ms: Vec<(String, String, i32, i64, Option<String>, i32, bool)> =
        sqlx::query_as(
            "SELECT code, season_key, need, reward_spark, item_key, \
                    item_qty, enabled \
               FROM arcade_milestones ORDER BY sort, code",
        )
        .fetch_all(db)
        .await
        .map_err(dberr)?;
    Ok(serde_json::json!({
        "quests": quests.iter().map(|(c, g, t, r, ik, q, en)| {
            serde_json::json!({ "code": c, "ref": g, "target": t,
                "reward": r, "item_key": ik, "item_qty": q, "enabled": en })
        }).collect::<Vec<_>>(),
        "milestones": ms.iter().map(|(c, s, n, r, ik, q, en)| {
            serde_json::json!({ "code": c, "season_key": s, "need": n,
                "reward": r, "item_key": ik, "item_qty": q, "enabled": en })
        }).collect::<Vec<_>>(),
    }))
}

/// 窗口内确定侧发放的**物品**折算价值（预算闸的另一半读数）
pub(super) async fn det_cost_value(
    db: &PgPool,
    uid: i64,
    window_days: i64,
) -> DomainResult<i64> {
    let v: i64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(sum(i.anchor * g.qty), 0)::bigint
          FROM arcade_item_grants g
          JOIN arcade_items i ON i.key = g.item_key
         WHERE g.side = 'det' AND g.user_id = $1
           AND g.granted_at >= now() - make_interval(days => $2::int)
        "#,
    )
    .bind(uid)
    .bind(window_days as i32)
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(v)
}

/// 坏引用门禁：名字与判据只写一份，玩家侧大厅与运营侧面板挂同一条。
/// 两处各写一遍的话，改名会让其中一侧悄悄失去这条闸（面板正是站长配坏东西的地方）。
pub(super) const REFS_GATE: &str =
    "确定侧奖励引用的物品都可用（停用会让领取当场失败）";

pub(super) async fn refs_gate(db: &PgPool) -> DomainResult<serde_json::Value> {
    let n = broken_reward_refs(db).await?;
    Ok(super::arcade_cfg::bad(
        REFS_GATE,
        n == 0,
        format!("{n} 条奖励行引用了停用或无折算价的物品"),
    ))
}

/// 坏引用体检：启用中的奖励行，引用了停用 / 无折算价 / 不存在的物品。
/// 这一条**能红**：站长在物品目录里停用一件正在被奖励引用的东西，就是普通动作。
pub(super) async fn broken_reward_refs(db: &PgPool) -> DomainResult<i64> {
    let n: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)::bigint FROM (
            SELECT item_key FROM arcade_quests
             WHERE enabled AND item_key IS NOT NULL
            UNION ALL
            SELECT item_key FROM arcade_milestones
             WHERE enabled AND item_key IS NOT NULL
        ) r
         WHERE NOT EXISTS (SELECT 1 FROM arcade_items i
                            WHERE i.key = r.item_key
                              AND i.enabled
                              AND (i.kind = 'cosmetic' OR i.anchor > 0))
        "#,
    )
    .fetch_one(db)
    .await
    .map_err(dberr)?;
    Ok(n)
}
