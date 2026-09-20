//! 站免池（M13）：状态/捐赠。
//! 从 economy_http.rs 按域拆出。

use super::ledger::spend_spark_tx;
use super::spend::SpendOutcome;
use crate::economy;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 站免池（M13） ============

#[get("/magic-pool")]
async fn pool_status(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let month = economy::pool_month(chrono::Utc::now());
    let row: Option<(String, i64, i64, bool)> = sqlx::query_as(
        "SELECT month, donated_total, goal, promo_started FROM magic_pool WHERE month = $1",
    )
    .bind(&month)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (m, donated, goal, started) =
        row.unwrap_or((month.clone(), 0, economy::MAGIC_POOL_GOAL, false));
    let top: Vec<(String, i64)> = sqlx::query_as(
        "SELECT u.username, sum(d.amount)::bigint FROM pool_donations d \
         JOIN users u ON u.id = d.user_id WHERE d.month = $1 \
         GROUP BY u.username ORDER BY 2 DESC LIMIT 10",
    )
    .bind(&month)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "month": m, "donated": donated, "goal": goal, "progress": if goal > 0 { donated as f64 / goal as f64 } else { 0.0 },
        "promo_started": started, "top_donors": top,
    })))
}

#[derive(Deserialize)]
struct DonateReq {
    amount: i64,
    /// 客户端幂等键（审计修复 P1：防网络重试双扣；缺省回退随机键保持兼容）
    #[serde(default)]
    idempotency_key: Option<String>,
}

#[post("/magic-pool/donate")]
async fn pool_donate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DonateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("捐赠金额必须为正".into()));
    }
    let month = economy::pool_month(chrono::Utc::now());
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("donate:{}:{}:{}", auth.id, month, k.trim()))
        .unwrap_or_else(|| {
            format!("donate:{}:{}:{}", auth.id, month, Uuid::new_v4())
        });
    // 审计修复（P1 非原子）：旧版 spend_spark 成功后 magic_pool / pool_donations 两段
    // INSERT 独立执行，失败即「钱扣了、池账与荣誉榜丢失」。改为单事务：扣款经
    // spend_spark_tx 与入池、流水三写同生共死，失败整体回滚（对照 funding_contribute）。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 幂等重放必须终止（P0）：重放时 spend 不扣款，继续累加池账/荣誉榜 = 捐赠翻倍，
    // 还可刷 donated_total 达标触发全站促销。对照 games scratch 的同款闸门。
    if !matches!(
        spend_spark_tx(
            &mut tx,
            auth.id,
            body.amount,
            "pool_donate",
            &idem,
            "pool",
            0,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔捐赠已受理，请勿重复提交".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
         ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
    )
    .bind(&month)
    .bind(body.amount)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("INSERT INTO pool_donations (user_id, amount, month) VALUES ($1, $2, $3)")
        .bind(auth.id)
        .bind(body.amount)
        .bind(&month)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "pool_donate", Some(body.amount))
        .await;
    Ok(ok(
        serde_json::json!({ "month": month, "donated": body.amount }),
    ))
}
