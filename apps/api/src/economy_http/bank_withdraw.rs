//! 银行定存取出（M11）。
//! 从 economy_http.rs 按域拆出。

use super::loans::bank_settings;
use super::spend::earn_spark_tx;
use crate::dto::ok;
use crate::economy::early_penalty;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

#[post("/bank/withdraw")]
async fn bank_withdraw(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WithdrawReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let d: Option<(
        i64,
        i64,
        i64,
        i64,
        String,
        i16,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, amount, interest, paid_interest, settle_mode, status, maturity_at \
         FROM bank_deposits WHERE id = $1 AND user_id = $2",
    )
    .bind(body.deposit_id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, amount, interest, paid_interest, mode, status, maturity_at)) =
        d
    else {
        return Err(DomainError::NotFound(body.deposit_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该存款已处理".into()));
    }
    let bs = bank_settings(&state.repo.db).await;
    let matured = chrono::Utc::now() >= maturity_at;
    // maturity：到期本息全额；提前支取扣手续费、不计息。
    // daily：利息已按日发放（paid_interest），到期只还本；提前支取追回未到期部分利息防套利。
    let (payable, penalty, clawback) = if mode == "daily" {
        if matured {
            (amount, 0i64, 0i64)
        } else {
            let p = early_penalty(amount, bs.penalty_bp);
            (amount - p - paid_interest, p, paid_interest)
        }
    } else if matured {
        (amount + interest, 0, 0)
    } else {
        let p = early_penalty(amount, bs.penalty_bp);
        (amount - p, p, 0)
    };
    if payable < 0 {
        return Err(DomainError::Validation(
            "已发利息超过本金与手续费之和，无法支取，请联系管理员".into(),
        ));
    }

    // P-639：`interest` 列原先在支取后保持「预计利息」原值，而 `/bank/deposits`
    // 不过滤 status、前端 `bank-actions-parts.tsx` 那行又无条件渲染
    // 「到期利息: {interest}」⇒ 提前支取（不计息）的单会在页面上一直挂着
    // 一笔从没发过的钱（现网实证：id=1 标 interest=12、罚金 25、实际到账
    // 4975=本金−罚金；id=2 同形 123/250/49750）。列的语义收到「本单实付利息」，
    // 与回执里 `interest_earned` 用同一个表达式，A 处扣钱与 B 处展示同尺。
    let paid_interest_final =
        if mode != "daily" && matured { interest } else { 0 };
    // 单事务（P1 撕裂窗口收口）：销单 CAS 与本息入账同生共死。旧版 CAS 先提交、
    // earn 失败靠回滚状态位补偿——进程崩溃窗口内 status=1 锁死重试路径 = 本息蒸发。
    // 锁序对齐 worker bank_fixed_mature（先销单后入账），且共用 withdraw:{id} 幂等键，
    // 与到期自动兑付天然互斥双付。
    let idem = format!("withdraw:{}", id);
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let updated = sqlx::query(
        "UPDATE bank_deposits SET status = 1, settled_at = now(), penalty = $2, \
         interest = $3, withdrawn_at = now() \
         WHERE id = $1 AND status = 0",
    )
    .bind(id)
    .bind(penalty)
    .bind(paid_interest_final)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 撕裂残留兜底：历史上「earn 已落账但状态位被回滚」的存单，此处 Replayed →
    // 仅补销单（本息已发过，不重复入账），属有意容忍的重放
    let earn_outcome =
        earn_spark_tx(&mut tx, auth.id, payable, "bank_withdraw", &idem)
            .await?;
    let _ = earn_outcome;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "paid": payable, "matured": matured, "penalty": penalty,
            "clawback": clawback,
            // 与落库的 interest 列同一个值（P-639 的「一处真源」）
            "interest_earned": paid_interest_final }),
    ))
}

#[derive(Deserialize)]
pub(super) struct WithdrawReq {
    pub(super) deposit_id: i64,
}
