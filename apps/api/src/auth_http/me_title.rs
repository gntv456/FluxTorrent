//! 自定义头衔（0292 商城审计 P1-4 消费端）：
//! 商店「自定义头衔」SKU 自 0292 起改为 unlock 语义——购买发一张
//! title_unlock 券，用户在 UserCP 自助设置一次文字后核销。
//! 此前该 SKU 预置 config.title，8 万买完头衔仍为空（无消费端）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct TitleReq {
    title: String,
}

/// 头衔状态自读（0295）：当前头衔 + 手上的可用解锁券与到期时间。
///
/// 为什么要有这个接口：商店 80000 魔力的「自定义头衔」SKU 只发券，
/// 核销端点 POST /me/title 自 0292 起就在，但**前端从未调用过它**
/// （grep 全仓 0 命中）⇒ 用户买完没有入口用，券只能静静过期。
/// 界面要说清「能改 / 不能改 / 何时失效」，必须先能读到状态。
#[get("/me/title")]
async fn me_title_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let row: Option<(Option<String>, Option<i64>, Option<String>)> =
        sqlx::query_as(
            "SELECT u.title, \
             (SELECT count(*)::bigint FROM user_vouchers v \
              WHERE v.user_id = u.id AND v.kind = 'title_unlock' \
                AND v.used_at IS NULL AND v.expires_at > now()) AS usable, \
             (SELECT max(v.expires_at)::text FROM user_vouchers v \
              WHERE v.user_id = u.id AND v.kind = 'title_unlock' \
                AND v.used_at IS NULL AND v.expires_at > now()) AS until \
             FROM users u WHERE u.id = $1",
        )
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((title, usable, until)) = row else {
        return Err(DomainError::Unauthorized);
    };
    Ok(ok(serde_json::json!({
        "title": title,
        "usable": usable.unwrap_or(0),
        "expires_at": until,
    })))
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

    // 核销一张未用的解锁券（行锁防并发双用）。
    // 0295（运营轮 P0-4）：必须同时判 expires_at——券默认 30 天有效（0001 起
    // user_vouchers.expires_at 有 DEFAULT），此前过期券照样能改头衔，
    // 等于「买 30 天，用一辈子」，与下载免流券（voucher_use.rs 判过期）不同口径。
    let voucher: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM user_vouchers \
         WHERE user_id = $1 AND kind = 'title_unlock' \
         AND used_at IS NULL AND expires_at > now() \
         ORDER BY id LIMIT 1 FOR UPDATE",
    )
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(voucher_id) = voucher else {
        return Err(DomainError::Validation(
            "需要一张可用的「自定义头衔」解锁券（已核销或已过期的券不能用）"
                .into(),
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
