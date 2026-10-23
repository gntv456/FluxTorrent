//! 求种建单（发布）：写口上限、悬赏冻结、客户端幂等重放。
//! 从 requests.rs 拆出（300 行门禁）：用文件内子模块而非顶层模块，
//! 避免与 content_http/mod.rs 的集中登记互踩。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::spend_spark_tx;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[post("/requests")]
pub(crate) async fn request_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RequestCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("求种标题不能为空".into()));
    }
    // 写口零上限（P-466 同款）：界面 maxLength 只是浏览器提示，绕开界面
    // 直接 POST 可存下任意大标题/简介，而列表会把它们原样带给每个看榜人。
    // 上限与 request-board-form.tsx 的 maxLength 同一枚数。
    if body.title.chars().count() > 200 {
        return Err(DomainError::Validation("求种标题最长 200 字".into()));
    }
    if body
        .descr
        .as_deref()
        .is_some_and(|d| d.chars().count() > 2000)
    {
        return Err(DomainError::Validation("求种简介最长 2000 字".into()));
    }
    if body.bounty < 0 {
        return Err(DomainError::Validation("悬赏不能为负".into()));
    }
    // P-632：旧幂等键是「新建单 id」拼出来的，等于没有——响应丢失后重试
    // 会再建一单、再扣一次悬赏。按商城口径收客户端键（体优先、头兜底），
    // 键带 uid 前缀防跨用户碰撞；重放直接回原单，不碰余额。
    let idem = crate::http::idempotency::body_or_header(&body.idempotency_key)
        .filter(|k| crate::http::idempotency::valid_shape(k))
        .map(|k| format!("req-bounty-idem:{}:{}", auth.id, k));
    if let Some(key) = &idem {
        let existing: Option<(i64, i64)> = sqlx::query_as(
            "SELECT r.id, r.bounty FROM spark_ledger l \
             JOIN requests r ON r.id = l.ref_id \
             WHERE l.idempotency_key = $1 AND l.user_id = $2 \
               AND l.kind = 'request_bounty' \
               AND l.ref_type = 'request_bounty' AND r.user_id = $2",
        )
        .bind(key)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some((id, frozen)) = existing {
            return Ok(ok(
                serde_json::json!({ "id": id, "bounty_frozen": frozen }),
            ));
        }
    }
    // 悬赏即时冻结（从余额划走，应种交付时转移给应种人）。
    // 审计修复（P0 吞钱）：扣费与建单此前非原子——INSERT 失败（超长/
    // 瞬断）时扣款已提交、且幂等键带随机 UUID 不可追回。改为：先建单
    // 占坑 → 扣费失败连单回滚（同一事务语义）。
    let id: i64 = if body.bounty > 0 {
        let mut tx = state
            .repo
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO requests (user_id, title, descr, bounty) \
             VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(auth.id)
        .bind(&body.title)
        .bind(&body.descr)
        .bind(body.bounty)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // spend_spark 自带事务（锁行→幂等→扣款→流水）；此处复用同一连接
        // 保证原子性。无客户端键（老客户端/无头）保留旧键形状：新单 id
        // 天然不重放。闸门为 #[must_use] 契约统一口径。
        let idem = idem
            .clone()
            .unwrap_or_else(|| format!("req-bounty:{}:{}", auth.id, id));
        if !matches!(
            spend_spark_tx(
                &mut tx,
                auth.id,
                body.bounty,
                "request_bounty",
                &idem,
                "request_bounty",
                id,
            )
            .await?,
            crate::economy_http::SpendOutcome::Spent
        ) {
            return Err(DomainError::Validation(
                "该笔请求已受理，请勿重复提交".into(),
            ));
        }
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        id
    } else {
        sqlx::query_scalar(
            "INSERT INTO requests (user_id, title, descr, bounty) \
             VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(auth.id)
        .bind(&body.title)
        .bind(&body.descr)
        .bind(body.bounty)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    };
    Ok(ok(
        serde_json::json!({ "id": id, "bounty_frozen": body.bounty }),
    ))
}

// P-374：P-314（写口不拒未知键）的漏网位。前端 request-board-form.tsx
//   只发 {title, descr, bounty, idempotency_key}，收紧未知键零风险。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestCreateReq {
    title: String,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    bounty: i64,
    /// 客户端幂等键（可选）：重试必须复用，成功后换新。
    #[serde(default)]
    idempotency_key: Option<String>,
}
