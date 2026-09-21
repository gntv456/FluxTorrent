//! M16 短讯发送（message_send）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct SendMsgReq {
    to: String,
    subject: String,
    body: String,
    /// 回复/转发的原信 id（回复自动加 Re: 前缀+引用原文；转发原样带文）
    #[serde(default)]
    reply_to: Option<i64>,
    #[serde(default)]
    forward_of: Option<i64>,
}

/// 发信（sendmessage.php 口径 + takemessage 的管理组豁免）：
/// - 接收限制：普通用户须过对方 accept_pm（yes/friends/no）+ 非黑名单；管理组（staff≥90）一律放行
/// - 防刷：普通用户 60s 一条（message_flood 表）；staff 不限
/// - 回复：subject 加 Re:/Re(n): 并引用原文；转发：原封带文给第三人
#[post("/messages")]
async fn message_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SendMsgReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let staff = auth.class_id >= 90;

    let target: Option<(i64, String, i32)> = sqlx::query_as(
        "SELECT id, accept_pm, \
         class_id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(&body.to)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((to_id, accept, _to_class)) = target else {
        return Err(DomainError::NotFound(0));
    };
    // 审计修复（P1）：不能给自己发私信（对照 medal_gift 的同款护栏）
    if to_id == auth.id {
        return Err(DomainError::Validation("不能给自己发私信".into()));
    }
    if body.subject.trim().is_empty() {
        return Err(DomainError::Validation("主题不能为空".into()));
    }
    // 接收限制（管理组豁免 —— takemessage.php staffmem 口径）
    if !staff {
        // 审计修复（P0）：黑名单拦截此前从未实现（注释声称校验却无查询），
        // 被拉黑者可继续私信目标用户。
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM friendships f \
             WHERE f.user_id = $2 AND f.friend_id = $1 AND f.list = 'black')",
        )
        .bind(auth.id)
        .bind(to_id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if blocked {
            return Err(DomainError::Validation("对方不接受你的私信".into()));
        }
        match accept.as_str() {
            "no" => {
                return Err(DomainError::Validation(
                    "对方仅接收管理组私信".into(),
                ))
            }
            "friends" => {
                let is_friend: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM \
                     friendships f WHERE (f.user_id=$1 AND f.friend_id=$2) OR \
                     (f.user_id=$2 AND f.friend_id=$1))",
                )
                .bind(auth.id)
                .bind(to_id)
                .fetch_one(&state.repo.db)
                .await
                .unwrap_or(false);
                if !is_friend {
                    return Err(DomainError::Validation(
                        "对方仅接收好友私信".into(),
                    ));
                }
            }
            _ => {}
        }
        // 防刷：60s 一条
        let last: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
            "SELECT last_sent_at FROM message_flood WHERE user_id = $1",
        )
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .flatten();
        if let Some(t) = last {
            if (chrono::Utc::now() - t).num_seconds() < 60 {
                return Err(DomainError::Validation(
                    "发送过于频繁，请 1 分钟后再试".into(),
                ));
            }
        }
    }

    // 回复：Re:/Re(n): 前缀 + 引用原文；转发：原样带文
    let mut subject = body.subject.trim().to_string();
    let mut text = body.body.clone();
    if let Some(rid) = body.reply_to {
        let orig: Option<(String, String)> = sqlx::query_as(
                        "SELECT subject, \
             body FROM messages WHERE id = $1 AND (receiver_id = $2 OR sender_id = $2)",
        )
        .bind(rid)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some((osub, obody)) = orig {
            let n = osub.match_indices("Re(").count();
            subject = if n == 0 && osub.starts_with("Re:") {
                format!("Re: {}", osub.trim_start_matches("Re: "))
            } else if n > 0 {
                format!("Re({}): {}", n + 1, osub)
            } else {
                format!("Re: {}", osub)
            };
            text = format!("{}\n\n———— 原信 ————\n{}", body.body, obody);
        }
    }
    if let Some(fid) = body.forward_of {
        let orig: Option<(String, String)> = sqlx::query_as(
                        "SELECT subject, \
             body FROM messages WHERE id = $1 AND (receiver_id = $2 OR sender_id = $2)",
        )
        .bind(fid)
        .bind(auth.id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if let Some((osub, obody)) = orig {
            subject = format!("Fw: {}", osub);
            text = obody;
        }
    }

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, location, saved, unread) \
         VALUES ($1, $2, $3, $4, 1, 1, true) RETURNING id",
    )
    .bind(auth.id)
    .bind(to_id)
    .bind(&subject)
    .bind(&text)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO message_flood (user_id, last_sent_at) VALUES ($1, now()) \
                 ON CONFLICT (user_id) DO UPDATE SET last_sent_at = now()",
    )
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .ok();
    Ok(ok(serde_json::json!({ "id": id })))
}
