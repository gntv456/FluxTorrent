//! 众筹出资与我的（0078）。
//! 从 economy_http.rs 按域拆出。

use super::spend::SpendOutcome;
use crate::dto::ok;
use crate::economy;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use uuid::Uuid;

/// 参与众筹（一人一项目可追投；扣款走 spend_spark 幂等，税入池、净额计入 raised）。
/// 退款时按实付全额退（税部分由站免池承担——池子本来就是回收通道）。
#[post("/fundings/contribute")]
async fn funding_contribute(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FundingContributeReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.amount <= 0 {
        return Err(DomainError::Validation("参与金额必须为正".into()));
    }
    let f: Option<(i64, i16)> = sqlx::query_as(
        "SELECT goal, status FROM fundings WHERE id = $1 AND ends_at > now()",
    )
    .bind(body.funding_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((goal, status)) = f else {
        return Err(DomainError::NotFound(body.funding_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("众筹已结束".into()));
    }
    // 幂等键必须带 uid 前缀（P0）：裸客户端键会跨用户碰撞——A 占用键 X 后，B 用 X
    // 参与会被判为重放而不扣款，contribs 照记、到期按全额退款 = 提现通道。
    // 与 bank/donate/games 全部同口径。
    let idem = body
        .idempotency_key
        .clone()
        .filter(|k| !k.trim().is_empty())
        .map(|k| format!("funding:{}:{}", auth.id, k.trim()))
        .ok_or(DomainError::Validation("缺少 idempotency_key".into()))?;
    // 税：基点可调（site_settings gift_tax_bp，缺省 500=5%）；0=免税
    let tax_bp: i32 = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'gift_tax_bp'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .and_then(|v: String| v.parse().ok())
    .unwrap_or(500);
    let tax = economy::gift_tax(body.amount, tax_bp);
    let net = body.amount - tax;
    // 幂等重放必须终止（P0）：重放时 spend 不扣款，若继续累加 raised/contribs =
    // 众筹虚增达标白嫖免费促销，到期还能按 contribs 全额退款。
    if !matches!(
        crate::economy_http::spend_spark(
            &state.repo.db,
            auth.id,
            body.amount,
            "funding",
            &idem,
            "funding",
            body.funding_id,
        )
        .await?,
        SpendOutcome::Spent
    ) {
        return Err(DomainError::Validation(
            "该笔参与已受理，请勿重复提交".into(),
        ));
    }
    // 参与记录 + 进度推进（审计 P1-5：旧版三段独立语句，扣款成功但 contribs 落库
    // 失败时该笔不在退款集合——worker 按 funding_contribs 逐行退，钱有去无回。
    // 现在两段进同一事务，任一失败整体回滚并冲销扣款。）
    let contrib_ok = async {
        let mut tx = state.repo.db.begin().await.map_err(|e| e.to_string())?;
        sqlx::query(
            "INSERT INTO funding_contribs (funding_id, user_id, \
             amount, tax) VALUES ($1, $2, $3, $4) ON CONFLICT (funding_id, \
             user_id) DO UPDATE SET amount = funding_contribs.amount + \
             EXCLUDED.amount, tax = funding_contribs.tax + EXCLUDED.tax, \
             created_at = now()",
        )
        .bind(body.funding_id)
        .bind(auth.id)
        .bind(body.amount)
        .bind(tax)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        sqlx::query("UPDATE fundings SET raised = raised + $2 WHERE id = $1")
            .bind(body.funding_id)
            .bind(net)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        tx.commit().await.map_err(|e| e.to_string())
    }
    .await;
    if let Err(why) = contrib_ok {
        let db = state.repo.db.clone();
        let uid = auth.id;
        let amount = body.amount;
        let idem2 = format!("funding_refund:{}", Uuid::new_v4());
        actix_web::rt::spawn(async move {
            let _ = crate::economy_http::earn_spark(
                &db,
                uid,
                amount,
                "funding_refund",
                &idem2,
            )
            .await;
        });
        return Err(DomainError::Validation(format!(
            "参与记录写入失败，已发起退款冲销：{why}"
        )));
    }
    // 税入站免池（与 pool_donate 同账：magic_pool + pool_donations）。
    // 账务口径：税不另记 spark_ledger——支出方的 -amount 流水已把含税全额记为回收，
    // 这里只入池账；若再向某个汇入账户记正流水会虚增 v_spark_flow_monthly 的 minted。
    if tax > 0 {
        let month = economy::pool_month(chrono::Utc::now());
        sqlx::query(
            "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
             ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
        )
        .bind(&month)
        .bind(tax)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query(
            "INSERT INTO pool_donations (user_id, amount, \
         month) VALUES ($1, $2, $3)",
        )
        .bind(auth.id)
        .bind(tax)
        .bind(&month)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "funding_contribute", Some(body.funding_id))
        .await;
    let raised: i64 =
        sqlx::query_scalar("SELECT raised FROM fundings WHERE id = $1")
            .bind(body.funding_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "funding_id": body.funding_id, "paid": body.amount, "tax": tax,
        "raised": raised, "goal": goal, "reached": raised >= goal,
    })))
}

/// 我的参与记录
#[get("/fundings/mine")]
async fn funding_my(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(i64, i64, i64, i16, i64)> = sqlx::query_as(
        "SELECT f.id, c.amount, c.tax, f.status, c.funding_id \
         FROM funding_contribs c JOIN fundings f ON f.id = c.funding_id \
         WHERE c.user_id = $1 ORDER BY c.created_at DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
pub(super) struct FundingContributeReq {
    pub(super) funding_id: i64,
    pub(super) amount: i64,
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}
