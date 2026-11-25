//! 奖励履约用例（两段式的第二段）：逐中奖行发放奖励。
//!
//! 退避：1/5/15/60/360 分钟（对齐 tangpt 插件）；重试耗尽（5 次）转
//! `exhausted` 等人工。幂等锚 = spark_ledger / user_medals / 内部 EXISTS
//! 预检，同一 winner 重放不双发。「已满足」（satisfied）不算失败：如勋章
//! 已持有、VIP 已永久——发不出去但用户已拥有等价物，不再重试。

use super::tx_err;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use serde_json::json;

/// 退避分钟表；索引 = attempt_count（第 5 次失败即 exhausted）。
const RETRY_MINUTES: [i64; 5] = [1, 5, 15, 60, 360];

/// 单个 winner 的履约。返回最终状态字符串。
pub(crate) async fn fulfill_one(
    state: &std::sync::Arc<AppState>,
    winner_id: i64,
) -> DomainResult<String> {
    let repo = &state.repo;
    // 锁履约行防并发双发；已终态直接返回
    let row: Option<(String, i32)> = sqlx::query_as(
        "SELECT status, attempt_count FROM fixed_lottery_fulfillments \
         WHERE winner_id = $1 FOR UPDATE",
    )
    .bind(winner_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(tx_err)?;
    let Some((status, attempts)) = row else {
        return Err(DomainError::NotFound(winner_id));
    };
    if matches!(status.as_str(), "fulfilled" | "satisfied" | "exhausted") {
        return Ok(status);
    }

    let ctx: Option<(i64, String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT w.user_id, p.reward_kind, p.name, p.reward_config \
         FROM fixed_lottery_winners w \
         JOIN fixed_lottery_prizes p ON p.id = w.prize_id \
         WHERE w.id = $1",
    )
    .bind(winner_id)
    .fetch_optional(&repo.db)
    .await
    .map_err(tx_err)?;
    let Some((uid, kind, pname, cfg)) = ctx else {
        return Err(DomainError::NotFound(winner_id));
    };
    let cid: i64 = sqlx::query_scalar(
        "SELECT campaign_id FROM fixed_lottery_winners WHERE id = $1",
    )
    .bind(winner_id)
    .fetch_one(&repo.db)
    .await
    .map_err(tx_err)?;

    let attempt = attempts + 1;
    match grant(state, uid, cid, winner_id, &kind, &cfg).await {
        Ok(final_status) => {
            let st = if final_status == "satisfied" {
                "satisfied"
            } else {
                "fulfilled"
            };
            sqlx::query(
                "UPDATE fixed_lottery_fulfillments \
                 SET status = $2, attempt_count = $3, last_error = NULL, \
                     next_retry_at = NULL, reward_snapshot = $4, \
                     fulfilled_at = now(), updated_at = now() \
                 WHERE winner_id = $1",
            )
            .bind(winner_id)
            .bind(st)
            .bind(attempt)
            .bind(&json!({"kind": kind, "name": pname}))
            .execute(&repo.db)
            .await
            .map_err(tx_err)?;
            sqlx::query(
                "UPDATE fixed_lottery_winners SET status = $2 WHERE id = $1",
            )
            .bind(winner_id)
            .bind(if st == "satisfied" {
                "satisfied"
            } else {
                "fulfilled"
            })
            .execute(&repo.db)
            .await
            .map_err(tx_err)?;
            repo.audit(Some(uid), "lottery.fulfilled", Some(cid)).await;
            Ok(st.to_string())
        }
        Err(e) => {
            let exhausted = attempt as usize >= RETRY_MINUTES.len();
            let next = if exhausted {
                None
            } else {
                let mins = RETRY_MINUTES[(attempt - 1) as usize];
                Some(chrono::Utc::now() + chrono::Duration::minutes(mins))
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
            .execute(&repo.db)
            .await
            .map_err(tx_err)?;
            Ok(st.to_string())
        }
    }
}

/// 按 reward_kind 分派发放。返回 "granted" 或 "satisfied"。
async fn grant(
    state: &std::sync::Arc<AppState>,
    uid: i64,
    cid: i64,
    winner_id: i64,
    kind: &str,
    cfg: &serde_json::Value,
) -> DomainResult<&'static str> {
    let db = &state.repo.db;
    let idem = |tag: &str| format!("flx-win:{cid}:{uid}:{tag}");
    let _ = winner_id;
    match kind {
        "spark" => {
            let amount =
                cfg.get("amount").and_then(|v| v.as_i64()).unwrap_or(0);
            if amount <= 0 {
                return Err(DomainError::Validation(
                    "魔力奖励金额未配置".into(),
                ));
            }
            crate::economy_http::earn_spark(
                db,
                uid,
                amount,
                "lottery",
                &idem("spark"),
            )
            .await?;
            Ok("granted")
        }
        "invite" | "rename_card" | "upload" | "vip" | "ad_free"
        | "rainbow_id" => {
            // 全部走 flux-shared 的商城同款效果口（shop 买与批量发同一把尺）
            let sk = match kind {
                "upload" => "upload_credit",
                other => other,
            };
            crate::economy_http::apply_item_effect(db, uid, sk, cfg, "lottery")
                .await?;
            Ok("granted")
        }
        "medal" => {
            let Some(mid) = cfg.get("medal_id").and_then(|v| v.as_i64()) else {
                return Err(DomainError::Validation(
                    "勋章奖励未配置 medal_id".into(),
                ));
            };
            let days = cfg.get("days").and_then(|v| v.as_i64());
            // 勋章不存在（配置错）必须 failed 可修复重试；已持有
            // （ON CONFLICT 吞行）才是 satisfied。两条路径都在 SQL 里
            // 判——单条 INSERT..SELECT 无法区分，先查勋章存在性。
            let medal_exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM medals WHERE id = $1)",
            )
            .bind(mid)
            .fetch_one(db)
            .await
            .map_err(tx_err)?;
            if !medal_exists {
                return Err(DomainError::Validation(
                    "勋章奖励的 medal_id 不存在".into(),
                ));
            }
            let n = sqlx::query(
                "INSERT INTO user_medals (user_id, medal_id, source, \
                     expires_at) \
                 SELECT $1, $2, 'lottery', \
                        CASE WHEN $3::int IS NOT NULL \
                             THEN now() \
                                  + make_interval(days => $3::int) \
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
            .await
            .map_err(tx_err)?
            .rows_affected();
            if n == 0 {
                return Ok("satisfied"); // 已持有：等价物在手，不重试
            }
            Ok("granted")
        }
        "item" => {
            let Some(key) = cfg.get("item_key").and_then(|v| v.as_str()) else {
                return Err(DomainError::Validation(
                    "物品奖励未配置 item_key".into(),
                ));
            };
            let idem = idem("item");
            // 幂等锚 arc arcade_item_grants.idem：重放不重发
            let n = sqlx::query(
                "INSERT INTO arcade_item_grants \
                     (item_key, user_id, game, side, idem) \
                 VALUES ($1, $2, 'lottery', 'det', $3) \
                 ON CONFLICT (idem) DO NOTHING",
            )
            .bind(key)
            .bind(uid)
            .bind(&idem)
            .execute(db)
            .await
            .map_err(|e| {
                // FK 违例 = 物品键不存在（配置错），可修复后重试
                if e.as_database_error()
                    .map(|d| d.is_foreign_key_violation())
                    .unwrap_or(false)
                {
                    DomainError::Validation("物品奖励的 item_key 不存在".into())
                } else {
                    DomainError::Internal(e.into())
                }
            })?
            .rows_affected();
            if n == 0 {
                return Ok("satisfied"); // 重放：已发过
            }
            Ok("granted")
        }
        "custom" => {
            // 自定义奖品不发资产：等中奖人提交领取资料走 claim 流程
            Ok("satisfied")
        }
        // 未知 kind 不做 format! 动态串（校验门禁口径：动态串只减不增），
        // 具体键名进日志，用户面走静态句
        other => {
            tracing::error!(kind = other, winner_id, "未知奖励类型");
            Err(DomainError::Validation("未知奖励类型".into()))
        }
    }
}
