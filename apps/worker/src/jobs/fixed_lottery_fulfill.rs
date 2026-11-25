//! 固定抽签·履约半场（0520 拆件守 300 行门禁）：RETRY_MINUTES 退避表 +
//! fulfill_one / grant_one / grant_effect。与 api 侧 services 镜像同一批
//! kind 与退避（改动须双向同步）；扫描入口在 fixed_lottery.rs。

use sqlx::PgPool;

/// 单 winner 履约（镜像 services::fulfill_one 的精简版：同一批 kind、
/// 同一退避表；worker 侧不落 audit，库侧账本自带幂等键可查）。
const RETRY_MINUTES: [i64; 5] = [1, 5, 15, 60, 360];

pub(crate) async fn fulfill_one(
    db: &PgPool,
    winner_id: i64,
) -> anyhow::Result<()> {
    let row: Option<(String, i32)> = sqlx::query_as(
        "SELECT status, attempt_count FROM fixed_lottery_fulfillments \
         WHERE winner_id = $1 FOR UPDATE",
    )
    .bind(winner_id)
    .fetch_optional(db)
    .await?;
    let Some((status, attempts)) = row else {
        anyhow::bail!("fulfillment {winner_id} missing");
    };
    if matches!(status.as_str(), "fulfilled" | "satisfied" | "exhausted") {
        return Ok(());
    }
    let ctx: Option<(i64, i64, String, String, serde_json::Value)> =
        sqlx::query_as(
            "SELECT w.user_id, w.campaign_id, p.reward_kind, p.name, \
                    p.reward_config \
             FROM fixed_lottery_winners w \
             JOIN fixed_lottery_prizes p ON p.id = w.prize_id \
             WHERE w.id = $1",
        )
        .bind(winner_id)
        .fetch_optional(db)
        .await?;
    let Some((uid, cid, kind, _pname, cfg)) = ctx else {
        anyhow::bail!("winner {winner_id} missing");
    };
    let attempt = attempts + 1;
    let grant = grant_one(db, uid, cid, winner_id, &kind, &cfg).await;
    match grant {
        Ok(final_st) => {
            let st = if final_st == "satisfied" {
                "satisfied"
            } else {
                "fulfilled"
            };
            sqlx::query(
                "UPDATE fixed_lottery_fulfillments \
                 SET status = $2, attempt_count = $3, last_error = NULL, \
                     next_retry_at = NULL, fulfilled_at = now(), \
                     updated_at = now() \
                 WHERE winner_id = $1",
            )
            .bind(winner_id)
            .bind(st)
            .bind(attempt)
            .execute(db)
            .await?;
            sqlx::query(
                "UPDATE fixed_lottery_winners SET status = $2 WHERE id = $1",
            )
            .bind(winner_id)
            .bind(st)
            .execute(db)
            .await?;
        }
        Err(e) => {
            let exhausted = attempt as usize >= RETRY_MINUTES.len();
            let next = if exhausted {
                None
            } else {
                Some(
                    chrono::Utc::now()
                        + chrono::Duration::minutes(
                            RETRY_MINUTES[(attempt - 1) as usize],
                        ),
                )
            };
            let st = if exhausted { "exhausted" } else { "failed" };
            sqlx::query(
                "UPDATE fixed_lottery_fulfillments \
                 SET status = $2, attempt_count = $3, last_error = $4, \
                     next_retry_at = $5, updated_at = now() \
                 WHERE winner_id = $1",
            )
            .bind(winner_id)
            .bind(st)
            .bind(attempt)
            .bind(e.to_string())
            .bind(next)
            .execute(db)
            .await?;
        }
    }
    Ok(())
}

async fn grant_one(
    db: &PgPool,
    uid: i64,
    cid: i64,
    winner_id: i64,
    kind: &str,
    cfg: &serde_json::Value,
) -> anyhow::Result<&'static str> {
    let idem = |t: &str| format!("flx-win:{cid}:{uid}:{t}");
    match kind {
        "spark" => {
            let amount =
                cfg.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
            if amount <= 0 {
                anyhow::bail!("spark amount not configured");
            }
            // 与 arcade_week_prize 同款单语句幂等入账（键存在即跳过）
            let n = sqlx::query(
                "INSERT INTO spark_ledger \
                    (id, user_id, amount, kind, idempotency_key, \
                     balance_after) \
                  SELECT nextval('spark_ledger_id_seq'), $1, $2, \
                    'lottery', $3, u.spark_balance + $2 \
                  FROM users u WHERE u.id = $1 \
                  AND NOT EXISTS (SELECT 1 FROM spark_ledger \
                                  WHERE idempotency_key = $3)",
            )
            .bind(uid)
            .bind(amount)
            .bind(idem("spark"))
            .execute(db)
            .await?
            .rows_affected();
            if n > 0 {
                sqlx::query(
                    "UPDATE users SET spark_balance = spark_balance + $2 \
                     WHERE id = $1",
                )
                .bind(uid)
                .bind(amount)
                .execute(db)
                .await?;
                Ok("granted")
            } else {
                Ok("satisfied") // 重放：已入账
            }
        }
        "invite" | "rename_card" | "vip" | "ad_free" | "rainbow_id" => {
            super::fixed_lottery_grant_effect::grant_effect(db, uid, kind, cfg)
                .await?;
            Ok("granted")
        }
        "upload" => {
            super::fixed_lottery_grant_effect::grant_effect(
                db,
                uid,
                "upload_credit",
                cfg,
            )
            .await?;
            Ok("granted")
        }
        "medal" => {
            let Some(mid) = cfg.get("medal_id").and_then(|v| v.as_i64()) else {
                anyhow::bail!("medal_id not configured");
            };
            let days = cfg.get("days").and_then(|v| v.as_i64());
            // 与 api 侧同修：勋章不存在=failed 可重试；已持有=satisfied
            let medal_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM medals WHERE id = $1)",
            )
            .bind(mid)
            .fetch_one(db)
            .await?;
            if !medal_exists {
                anyhow::bail!("medal_id {} does not exist", mid);
            }
            let n = sqlx::query(
                "INSERT INTO user_medals (user_id, medal_id, source, \
                     expires_at) \
                 SELECT $1, $2, 'lottery', \
                        CASE WHEN $3::int IS NOT NULL \
                             THEN now() + make_interval(days => $3::int) \
                             ELSE now() + make_interval( \
                                      days => m.duration_days) \
                        END \
                 FROM medals m WHERE m.id = $2 \
                 ON CONFLICT (user_id, medal_id) DO NOTHING",
            )
            .bind(uid)
            .bind(mid)
            .bind(days.map(|d| d as i32))
            .execute(db)
            .await?
            .rows_affected();
            if n == 0 {
                return Ok("satisfied");
            }
            Ok("granted")
        }
        "item" => {
            let Some(key) = cfg.get("item_key").and_then(|v| v.as_str()) else {
                anyhow::bail!("item_key not configured");
            };
            let n = sqlx::query(
                "INSERT INTO arcade_item_grants \
                     (item_key, user_id, game, side, idem) \
                 VALUES ($1, $2, 'lottery', 'det', $3) \
                 ON CONFLICT (idem) DO NOTHING",
            )
            .bind(key)
            .bind(uid)
            .bind(idem("item"))
            .execute(db)
            .await?
            .rows_affected();
            if n == 0 {
                return Ok("satisfied");
            }
            Ok("granted")
        }
        "custom" => Ok("satisfied"),
        other => anyhow::bail!("unknown reward kind {other}"),
    }
}
