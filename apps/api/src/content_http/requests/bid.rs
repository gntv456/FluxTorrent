//! 求种加价（0417）：楼主续押与他人加价共用一口。
//! 从 requests.rs 拆出（行数门禁）：与 create.rs 同为文件内子模块。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::spend_spark_tx;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[post("/requests/{id}/bid")]
pub(crate) async fn request_bid(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<BidReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let request_id = path.into_inner();
    if body.amount <= 0 {
        return Err(DomainError::Validation("加价金额必须为正".into()));
    }
    if body.amount > 1_000_000 {
        return Err(DomainError::Validation("单笔加价上限 1,000,000".into()));
    }
    // 求种单必须存在且未完结——完结后加价等于把钱冻进没人能领的池子。
    // 连楼主带旁人一律拒绝（旁人加价看不到状态变化更冤）。
    let r: Option<(i64, i16)> = sqlx::query_as(
        "SELECT user_id, status FROM requests WHERE id = $1",
    )
    .bind(request_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((requester, status)) = r else {
        return Err(DomainError::NotFound(request_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该求种已处理，无法加价".into()));
    }
    // 幂等（与建单同口径）：客户端键体优先、头兜底，带 uid 前缀。
    // 重放 = 那笔加价早已入池，直接回当前池额，不碰余额。
    let idem = crate::http::idempotency::body_or_header(&body.idempotency_key)
        .filter(|k| crate::http::idempotency::valid_shape(k))
        .map(|k| format!("req-bid-idem:{}:{}", auth.id, k));
    if let Some(key) = &idem {
        let existing: Option<i64> = sqlx::query_scalar(
            "SELECT r.bounty FROM spark_ledger l \
             JOIN requests r ON r.id = l.ref_id \
             WHERE l.idempotency_key = $1 AND l.user_id = $2 \
               AND l.kind = 'request_bid' \
               AND l.ref_type = 'request_bid' AND r.id = $3",
        )
        .bind(key)
        .bind(auth.id)
        .bind(request_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some(total) = existing {
            return Ok(ok(
                serde_json::json!({ "request_id": request_id, "bounty_total": total, "replayed": true }),
            ));
        }
    }
    // 冻结与入池同事务（与建单 P0 吞钱修复同构）：明细行写失败连扣款一起回滚。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let idem = idem.unwrap_or_else(|| {
        // 无客户端键：用「单 + 人 + 池内笔数」构造——同人对同一单的连续加价
        // 键不同（笔数递增），网络重试则因事务里已落行的笔数变化不再命中，
        // 由下方 spend 幂等层兜住真双击。
        format!("req-bid:{}:{}:{}:c", auth.id, request_id, chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0))
    });
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.amount,
            "request_bid",
            &idem,
            "request_bid",
            request_id,
        )
        .await?,
        crate::economy_http::SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation("该笔加价已受理，请勿重复提交".into()));
    }
    sqlx::query(
        "INSERT INTO request_bids (request_id, user_id, amount, idem_key) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(request_id)
    .bind(auth.id)
    .bind(body.amount)
    .bind(&idem)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "UPDATE requests SET bounty = bounty + $2 WHERE id = $1 \
         RETURNING bounty",
    )
    .bind(request_id)
    .bind(body.amount)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 通知楼主有人加价（自己续押不通知）。失败只出声不阻断——加价已落库。
    if requester != auth.id {
        let pm = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body, kind, params) \
             SELECT NULL, $1, '你的求种有人加价', \
                    format('求种 #%s 悬赏池新增 %s 魔力，当前总额 %s。', \
                           $2::text, $3::text, $4::text), \
                    'request_bid', \
                    jsonb_build_object('request_id', $2, 'amount', $3, 'total', $4) \
             WHERE u_notice_enabled($1, 'request_bid')",
        )
        .bind(requester)
        .bind(request_id)
        .bind(body.amount)
        .bind(total)
        .execute(&state.repo.db)
        .await;
        if let Err(e) = pm {
            tracing::warn!(?e, request = request_id, "求种加价通知写入失败");
        }
    }
    Ok(ok(
        serde_json::json!({ "request_id": request_id, "bounty_total": total }),
    ))
}

// P-374 口径：前端只发 {amount, idempotency_key}，收紧未知键。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BidReq {
    amount: i64,
    /// 客户端幂等键（可选）：重试必须复用，成功后换新。
    #[serde(default)]
    idempotency_key: Option<String>,
}
