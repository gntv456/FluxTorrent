//! 自定义头衔（0292 商城审计 P1-4 消费端）：
//! 商店「自定义头衔」SKU 自 0292 起改为 unlock 语义——购买发一张
//! title_unlock 券，用户在 UserCP 自助设置一次文字后核销。
//! 此前该 SKU 预置 config.title，8 万买完头衔仍为空（无消费端）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct TitleReq {
    title: String,
}

/// 设置头衔（核销 title_unlock 券）。规则与 shop_effects 直设分支同口径：
/// 非空、≤30 字符；清空（空串）视为「撤销头衔」，同样消耗券。
#[post("/me/title")]
async fn me_title_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TitleReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let t = body.title.trim();
    if t.chars().count() > 30 {
        return Err(DomainError::Validation("头衔最多 30 个字符".into()));
    }

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 核销一张未用的解锁券（行锁防并发双用）
    let voucher: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM user_vouchers \
         WHERE user_id = $1 AND kind = 'title_unlock' \
         AND used_at IS NULL ORDER BY id LIMIT 1 FOR UPDATE",
    )
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(voucher_id) = voucher else {
        return Err(DomainError::Validation(
            "需要先在商店购买「自定义头衔」（或已使用过）".into(),
        ));
    };

    sqlx::query("UPDATE users SET title = $2 WHERE id = $1")
        .bind(auth.id)
        .bind(if t.is_empty() { None } else { Some(t) })
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("UPDATE user_vouchers SET used_at = now() WHERE id = $1")
        .bind(voucher_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "me_title_set", None).await;
    Ok(ok(serde_json::json!({ "title": t })))
}
