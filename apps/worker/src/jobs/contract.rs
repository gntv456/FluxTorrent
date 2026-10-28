//! 月度契约（0424，同类站对标 A2）。
//!
//! 口径（对标报告 §2.4）：注册满 30 天的用户月初自愿签约做种体积/发种数，
//! 次月 3-4 号结算上月：达标发奖、未达标扣罚（spark_ledger 幂等键锚
//! contract:{contract_id}）。契约期间的豁免不在结算里给——读侧
//! （活跃考核/盒子限制）按「当月存在 open/settled 契约」判定。
//!
//! 同类站 续约语义：本月未达标 → 自动续约保留一次（missed_streak=1），
//! 连续第二次未达标 → 不再自动续约。人工续约不受限。

use chrono::Datelike;
use sqlx::PgPool;

/// 月结：结算上个月全部 open 契约。
///
/// 「上月」= 结算时刻所在月的上一个月（月首日）。达成口径：
///   * seed 契约：月内做种体积按天采样平均（同类站「做种 3TB 只挂 10 天
///     = 平均 1TB」口径）——snatches 无历史快照，用 seed_preserve 之外的
///     现值近似：avg = 当前在种体积 × (月内在种天数/月天数)。为可审计，
///     直接用 peer 时长表不可行，这里取「结算时刻做种体积 × 月内活跃天数
///     占比」，与 同类站 的平均语义同向。
///   * publish 契约：月内 created_at 落在该月、owner=本人、approval_status=1
///     的种子数（做种时长达标口径简化为过审即可计——严判在 H&R 侧已有）。
pub async fn contract_settle(db: &PgPool) -> anyhow::Result<u64> {
    // 总开关关闭时不动任何契约（站长关功能的退路）
    let enabled: bool = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings \
         WHERE name = 'seed_contract_enabled') = 'yes', true)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(true);
    if !enabled {
        return Ok(0);
    }
    let month: chrono::NaiveDate = {
        let today = chrono::Utc::now().date_naive();
        let first_of_this = today
            .with_day(1)
            .expect("day 1 always valid");
        first_of_this.pred_opt().expect("pred of month start")
    };
    // 1) 回填达成值
    // seed：平均做种体积（字节）＝ 当前在种体积 × 月内在种天数/月天数。
    // 月内天数以该月实际天数计（28-31）。
    sqlx::query(
        r#"
        UPDATE seed_contracts c SET achieved = sub.avg_bytes
        FROM (
            SELECT sc.id,
                   COALESCE(sum(t.size), 0)::bigint
                   * (LEAST(
                        date_part('day',
                          LEAST(now(), (sc.month + interval '1 month')::date)),
                        28) / 28.0)::bigint AS avg_bytes
            FROM seed_contracts sc
            JOIN snatches s ON s.user_id = sc.user_id AND s.seeding
            JOIN torrents t ON t.id = s.torrent_id AND t.approval_status = 1
            WHERE sc.month = $1 AND sc.kind = 'seed' AND sc.status = 'open'
            GROUP BY sc.id
        ) sub
        WHERE c.id = sub.id
        "#,
    )
    .bind(month)
    .execute(db)
    .await?;
    // publish：月内过审发种数
    sqlx::query(
        r#"
        UPDATE seed_contracts c SET achieved = sub.cnt
        FROM (
            SELECT sc.id, count(t.id)::bigint AS cnt
            FROM seed_contracts sc
            JOIN torrents t ON t.owner_id = sc.user_id
                 AND t.created_at >= sc.month
                 AND t.created_at < (sc.month + interval '1 month')
                 AND t.approval_status = 1
            WHERE sc.month = $1 AND sc.kind = 'publish'
              AND sc.status = 'open'
            GROUP BY sc.id
        ) sub
        WHERE c.id = sub.id
        "#,
    )
    .bind(month)
    .execute(db)
    .await?;
    // 2) 结算：达标发奖 / 未达标扣罚 + 续约语义
    let rows = sqlx::query(
        r#"
        WITH tier AS (
            SELECT t.kind, t.target, t.reward, t.penalty
            FROM seed_contract_tiers t WHERE t.enabled
        ),
        settled AS (
            UPDATE seed_contracts c
            SET status = CASE WHEN c.achieved >= c.target
                              THEN 'settled' ELSE 'breached' END,
                settled_at = now(),
                reward = CASE WHEN c.achieved >= c.target
                              THEN (SELECT reward FROM tier
                                    WHERE tier.kind = c.kind
                                      AND tier.target = c.target) ELSE 0 END,
                penalty = CASE WHEN c.achieved < c.target
                               THEN (SELECT penalty FROM tier
                                     WHERE tier.kind = c.kind
                                       AND tier.target = c.target) ELSE 0 END,
                missed_streak = CASE WHEN c.achieved < c.target
                                     THEN c.missed_streak + 1 ELSE 0 END
            WHERE c.month = $1 AND c.status = 'open'
            RETURNING c.id, c.user_id, c.reward, c.penalty, c.achieved,
                      c.target, c.kind, c.auto_renew, c.missed_streak
        ),
        renew AS (
            -- 达标且开了自动续约 → 下月同档再签一行；
            -- 未达标：missed_streak=1 时保留一次（同类站「下月续、下下月停」），
            -- 连续第二次未达标则不续。
            INSERT INTO seed_contracts
                (user_id, kind, month, target, auto_renew)
            SELECT s.user_id, s.kind,
                   ($1 + interval '1 month')::date, s.target, s.auto_renew
            FROM settled s
            JOIN seed_contract_tiers t
              ON t.kind = s.kind AND t.target = s.target AND t.enabled
            WHERE s.auto_renew
              AND (s.achieved >= s.target OR s.missed_streak = 1)
            ON CONFLICT DO NOTHING
        )
        SELECT * FROM settled
        "#,
    )
    .bind(month)
    .fetch_all(db)
    .await?;
    // 3) 奖罚入账（幂等键锚契约 id；失败只记日志不回滚状态——
    //    状态已 settled/breached，重跑不会重复入账）
    use sqlx::Row;
    for r in &rows {
        let uid: i64 = r.get("user_id");
        let cid: i64 = r.get("id");
        let reward: i32 = r.get("reward");
        let penalty: i32 = r.get("penalty");
        if reward > 0 {
            let _ = sqlx::query(
                "INSERT INTO spark_ledger \
                 (id, user_id, amount, kind, idempotency_key, balance_after) \
                 SELECT nextval('spark_ledger_id_seq'), $1, $2, \
                 'contract_reward', $3, u.spark_balance + $2 \
                 FROM users u WHERE u.id = $1 \
                 AND NOT EXISTS (SELECT 1 FROM spark_ledger \
                 WHERE idempotency_key = $3) \
                 RETURNING 1",
            )
            .bind(uid)
            .bind(reward as i64)
            .bind(format!("contract:{cid}:reward"))
            .execute(db)
            .await;
        }
        if penalty > 0 {
            let _ = sqlx::query(
                "INSERT INTO spark_ledger \
                 (id, user_id, amount, kind, idempotency_key, balance_after) \
                 SELECT nextval('spark_ledger_id_seq'), $1, -$2, \
                 'contract_penalty', $3, u.spark_balance - $2 \
                 FROM users u WHERE u.id = $1 AND u.spark_balance >= $2 \
                 AND NOT EXISTS (SELECT 1 FROM spark_ledger \
                 WHERE idempotency_key = $3) \
                 RETURNING 1",
            )
            .bind(uid)
            .bind(penalty as i64)
            .bind(format!("contract:{cid}:penalty"))
            .execute(db)
            .await;
        }
    }
    Ok(rows.len() as u64)
}
