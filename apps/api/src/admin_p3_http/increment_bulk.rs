//! 批量发放（好学 increment-bulk.php 口径）：火花/上传量/邀请/补签卡 ×（等级|职务|指定用户）。
//! 从 admin_p3_http.rs 按域拆出；校验与目标选择见 increment_targets.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::increment_targets::{
    increment_bulk_targets, increment_bulk_validate,
};
use super::staff;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub(super) struct IncrementBulkReq {
    /// spark | uploaded | invite | resub_card
    pub(super) kind: String,
    /// spark/invite/resub_card 为数量；uploaded 为 GB（正加负减）
    pub(super) amount: i64,
    /// 目标等级（多选）；与 roles/user_ids 至少其一
    #[serde(default)]
    pub(super) classes: Vec<i32>,
    /// 目标职务（多选）
    #[serde(default)]
    pub(super) roles: Vec<String>,
    /// 指定用户（优先于 classes/roles）
    #[serde(default)]
    pub(super) user_ids: Vec<i64>,
    /// 临时邀请有效期（天，1-365）：仅 kind=invite 时有效——直接生成 N 天到期的邀请码
    /// （好学 tmp_invites 口径）；缺省走 quota_extra 配额
    #[serde(default)]
    pub(super) days: Option<i32>,
    /// PM 通知（可选；空则不发）
    #[serde(default)]
    pub(super) subject: Option<String>,
    #[serde(default)]
    pub(super) body: Option<String>,
    /// 发送者：self = 操作者，system = 系统私信（sender NULL）
    #[serde(default)]
    pub(super) sender: Option<String>,
    /// kind=medal：勋章 id（0204）
    #[serde(default)]
    pub(super) medal_id: Option<i64>,
    /// kind=item：道具 id（0204）
    #[serde(default)]
    pub(super) item_id: Option<i64>,
}

#[post("/admin/increment-bulk")]
async fn increment_bulk(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<IncrementBulkReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    increment_bulk_validate(&state, &auth, &body).await?;
    let db = &state.repo.db;
    let targets: Vec<i64> = increment_bulk_targets(db, &body).await?;
    let batch_id = uuid::Uuid::new_v4().simple().to_string();
    let sender_id: Option<i64> = if body.sender.as_deref() == Some("system") {
        None
    } else {
        Some(auth.id)
    };

    // 分批执行（每批 500，避免长事务）
    let mut affected: u64 = 0;
    for chunk in targets.chunks(500) {
        match body.kind.as_str() {
            "spark" => {
                // 余额权威在流水：逐用户落 spark_ledger（负数扣减下限 0）
                for uid in chunk {
                    let idem = format!("increment_bulk:{batch_id}:{uid}");
                    if body.amount > 0 {
                        crate::economy_http::earn_spark(
                            db,
                            *uid,
                            body.amount,
                            "increment_bulk",
                            &idem,
                        )
                        .await?;
                    } else {
                        // 审计修复（P1 账本不变量）：旧实现按原始 amount 落流水、余额却
                        // GREATEST(0,...) 截断 —— 用户余额 500 扣 1000 时流水记 -1000、
                        // 余额变 0，sum(ledger) ≠ balance 从此失真。改为 CTE 里锁行取前值，
                        // 流水按「前值-后值」的真实差额落（截断时 |流水| < |amount|）。
                        let before_after: Option<(i64, i64)> = sqlx::query_as(
                            "WITH prev AS (SELECT spark_balance AS b FROM users WHERE id = $1 FOR UPDATE), \
                              upd AS (UPDATE users SET spark_balance = GREATEST(0, spark_balance + $2) \
                                      WHERE id = $1 RETURNING spark_balance AS a) \
                             SELECT prev.b, upd.a FROM prev, upd",
                        )
                        .bind(uid)
                        .bind(body.amount)
                        .fetch_optional(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                        let Some((bal_before, bal_after)) = before_after else {
                            continue;
                        };
                        let actual_delta = bal_after - bal_before; // ≤0；截断时比 amount 接近 0
                        sqlx::query(
                            "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
                             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'increment_bulk', $3, $4)",
                        )
                        .bind(uid)
                        .bind(actual_delta)
                        .bind(&idem)
                        .bind(bal_after)
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                    }
                    affected += 1;
                }
            }
            "uploaded" => {
                let bytes = body.amount * 1024 * 1024 * 1024;
                // 审计修复（P0 错账，与 /admin/amountupload 同病）：uploaded 权威在
                // traffic_ledger，reconcile 会重算 users.uploaded=sum(ledger)，裸 UPDATE
                // 的手工加量会被对账静默清零。此处同步落差额流水。
                affected += sqlx::query(
                    "WITH targets AS ( \
                        SELECT id, uploaded FROM users WHERE id = ANY($1) FOR UPDATE \
                     ), upd AS ( \
                        UPDATE users u SET uploaded = GREATEST(0, u.uploaded + $2) \
                        FROM targets t WHERE u.id = t.id \
                        RETURNING u.id, GREATEST(0, t.uploaded + $2) - t.uploaded AS delta \
                     ) \
                     INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                     SELECT nextval('traffic_ledger_id_seq'), id, 0, delta, 0, now() FROM upd WHERE delta <> 0",
                )
                .bind(chunk)
                .bind(bytes)
                .execute(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .rows_affected();
            }
            "invite" => {
                if let Some(days) = body.days {
                    // 临时邀请（好学 tmp_invites 口径）：直接生成 N 天有效期的邀请码，
                    // 不走 quota_extra——码已在手，到期自动失效（status 由 worker 置 2）
                    let expires = chrono::Utc::now()
                        + chrono::Duration::days(days.clamp(1, 365) as i64);
                    for uid in chunk {
                        for _ in 0..body.amount {
                            let code = crate::domain::new_invite_code();
                            sqlx::query(
                                "INSERT INTO \
                                 invites (inviter_id, code, expires_at) VALUES \
                                 ($1, $2, $3)",
                            )
                            .bind(uid)
                            .bind(&code)
                            .bind(expires)
                            .execute(db)
                            .await
                            .map_err(|e| DomainError::Internal(e.into()))?;
                            affected += 1;
                        }
                    }
                } else {
                    // 普通发放：正数增发 quota_extra；负数回收（下限 0），与单用户 adjust 口径一致
                    affected += sqlx::query(
                        "UPDATE users SET quota_extra \
                         = GREATEST(0, quota_extra + $2) WHERE id = ANY($1)",
                    )
                    .bind(chunk)
                    .bind(body.amount)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .rows_affected();
                }
            }
            "resub_card" => {
                // 补签卡：给每人 amount 张（0 价订单入包，复用 grant-item 的背包语义）
                // 道具字典种子 kind=makeup_card；gaps_http 补签流程引用 resub_card（历史不一致），
                // 此处两种都认，优先 makeup_card
                let item: Option<(i64, serde_json::Value)> = sqlx::query_as(
                                        "SELECT id, \
                     config FROM shop_items WHERE kind IN ('makeup_card','resub_card') AND active = true ORDER BY kind = 'makeup_card' DESC, \
                     id LIMIT 1",
                )
                .fetch_optional(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                let Some((item_id, config)) = item else {
                    return Err(DomainError::Validation(
                        "商店缺少可用的补签卡道具（makeup_card）".into(),
                    ));
                };
                for uid in chunk {
                    for _ in 0..body.amount {
                        let idem = format!(
                            "increment_bulk:{batch_id}:{uid}:{}",
                            uuid::Uuid::new_v4().simple()
                        );
                        sqlx::query(
                            "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
                             VALUES ($1, $2, 0, $3, $4)",
                        )
                        .bind(uid)
                        .bind(item_id)
                        .bind(&idem)
                        .bind(&config)
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                        affected += 1;
                    }
                }
            }
            // 0204：勋章批量发放——与单发（user_grant_medal）同语义：
            // source='admin'、有效期随 medals.duration_days、PK 冲突跳过（天然幂等）。
            // 等级护栏：跳过不低于操作者的目标（对齐 users_batch 逐人跳过口径）。
            "medal" => {
                let medal_id = body.medal_id.ok_or_else(|| {
                    DomainError::Validation("需选择勋章".into())
                })?;
                for uid in chunk {
                    let skipped: bool = sqlx::query_scalar(
                        "SELECT class_id >= $2 FROM users WHERE id = $1",
                    )
                    .bind(uid)
                    .bind(auth.class_id)
                    .fetch_one(db)
                    .await
                    .unwrap_or(true);
                    if skipped {
                        continue;
                    }
                    let n = sqlx::query(
                        "INSERT INTO user_medals (user_id, medal_id, source, expires_at) \
                         SELECT $1, $2, 'admin', now() + make_interval(days => m.duration_days) \
                         FROM medals m WHERE m.id = $2 \
                         ON CONFLICT (user_id, medal_id) DO NOTHING",
                    )
                    .bind(uid)
                    .bind(medal_id)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?
                    .rows_affected();
                    affected += n;
                }
            }
            // 0204：道具批量发放——按 grant-item 的 kind 分流：
            // 即时类（upload_credit/gift_spark/invite/temp_invite）直接生效；
            // 背包类逐人逐张 0 价单入 shop_orders（独立幂等键，UNIQUE 约束防重）。
            "item" => {
                let item_id = body.item_id.ok_or_else(|| {
                    DomainError::Validation("需选择道具".into())
                })?;
                let item: Option<(String, serde_json::Value)> =
                    sqlx::query_as(
                        "SELECT kind, config FROM shop_items WHERE id = $1 AND active = true",
                    )
                    .bind(item_id)
                    .fetch_optional(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                let Some((kind, config)) = item else {
                    return Err(DomainError::NotFound(item_id));
                };
                match kind.as_str() {
                    "invite" | "temp_invite" => {
                        affected += sqlx::query(
                            "UPDATE users SET quota_extra = quota_extra + $2 WHERE id = ANY($1) AND status < 2",
                        )
                        .bind(chunk)
                        .bind(body.amount)
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?
                        .rows_affected();
                    }
                    "upload_credit" => {
                        let gb = config
                            .get("gb")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(1)
                            * body.amount;
                        affected += sqlx::query(
                            "UPDATE users SET uploaded = uploaded + $2::bigint * 1073741824 WHERE id = ANY($1) AND status < 2",
                        )
                        .bind(chunk)
                        .bind(gb)
                        .execute(db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?
                        .rows_affected();
                    }
                    "gift_spark" => {
                        let amt = config
                            .get("amount")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0)
                            * body.amount;
                        for uid in chunk {
                            let idem = format!(
                                "increment_bulk:{batch_id}:{uid}:{}",
                                uuid::Uuid::new_v4().simple()
                            );
                            crate::economy_http::earn_spark(
                                db,
                                *uid,
                                amt,
                                "increment_bulk",
                                &idem,
                            )
                            .await?;
                            affected += 1;
                        }
                    }
                    _ => {
                        for uid in chunk {
                            for _ in 0..body.amount {
                                let idem = format!(
                                    "increment_bulk:{batch_id}:{uid}:{}",
                                    uuid::Uuid::new_v4().simple()
                                );
                                sqlx::query(
                                    "INSERT INTO shop_orders (user_id, item_id, price, idempotency_key, config_snapshot) \
                                     VALUES ($1, $2, 0, $3, $4)",
                                )
                                .bind(uid)
                                .bind(item_id)
                                .bind(&idem)
                                .bind(&config)
                                .execute(db)
                                .await
                                .map_err(|e| DomainError::Internal(e.into()))?;
                                affected += 1;
                            }
                        }
                    }
                }
            }
            _ => unreachable!(),
        }

        // PM 通知（可选）
        if let (Some(subject), Some(text)) =
            (body.subject.as_deref(), body.body.as_deref())
        {
            if !subject.trim().is_empty() && !text.trim().is_empty() {
                for uid in chunk {
                    sqlx::query(
                        "INSERT INTO messages \
                         (sender_id, receiver_id, subject, body) VALUES ($1, \
                         $2, $3, $4)",
                    )
                    .bind(sender_id)
                    .bind(uid)
                    .bind(subject.trim())
                    .bind(text.trim())
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
    }

    state
        .repo
        .audit(
            Some(auth.id),
            &format!("increment_bulk.{}", body.kind),
            None,
        )
        .await;
    Ok(ok(serde_json::json!({
        "affected": affected as i64,
        "targets": targets.len() as i64,
        "kind": body.kind,
        "amount": body.amount,
    })))
}
