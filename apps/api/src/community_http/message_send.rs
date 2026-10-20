//! M16 短讯发送（message_send）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize, utoipa::ToSchema)]
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
#[utoipa::path(
    post,
    path = "/api/v1/messages",
    tag = "社区/消息",
    description = "发站内信（可带 passkey 索取链接）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/messages")]
pub(crate) async fn message_send(
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
        // 收件人打错是这条链上最常见的失误，回「资源不存在」会员看不懂
        // （同函数另两条护栏都是中文整句：不能给自己发 / 主题不能为空）。
        // 状态 >=2（已注销/封禁）与「查无此人」在这里合并成一句，
        // 不额外区分，免得给枚举用户名留口子。
        return Err(DomainError::Validation(format!(
            "收件人不存在或已注销：{}",
            body.to.trim()
        )));
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
                // P-295：这道屏障必须读 `friendships.list`（旧写法两向都算、
                // 又不带 list ⇒ ①单向 pending 申请也算好友，「仅接收好友私信」
                // 形同虚设（EB3 修前实测 200 且真落一行）；②A 拉黑 B 后 A 自己
                // 还能发）。方向仍留双向：friend.rs 有一条隐性路径只翻转申请方
                // 那一行（注释却写「双向转正」，已按 P-296 入档待站方修），只按
                // 接收方判会把真好友误伤——EB4 正控专门盯这一条。
                // 两个方向都算、又不带 list 条件 ⇒ ①单向 pending 申请（A 申请
                // B、B 从未接受）也算好友，选了「仅接收好友私信」的 B 照样被 A
                // 私信，这道屏障形同虚设；②A 把 B 拉黑后 A 自己仍能发
                // （(A,B,'black') 也命中那个 OR）。friend.rs 的注释写明
                // 「好友关系必须是双向两行」正是为了守这道屏障，而屏障自己的
                // 判定没读那个列。
                let is_friend: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM friendships f \
                     WHERE ((f.user_id = $1 AND f.friend_id = $2) \
                            OR (f.user_id = $2 AND f.friend_id = $1)) \
                       AND f.list = 'friend')",
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
        // 禁言同样挡住站短（0285）：骚扰多发生在私信，禁言不该只管论坛。
        if crate::community_http::is_muted(&state.repo.db, auth.id).await {
            return Err(DomainError::Validation(
                "账号已被禁言，暂不能发私信".into(),
            ));
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

    // P-441：「保存短讯至发件箱」(users.save_pm，0024，默认 FALSE) 此前全站
    // 无人读，而发件箱按 `saved = 1` 过滤（message.rs 的 outbox 口）⇒ 关着
    // 开关的人每封照样留副本。现网账面：2961 人 save_pm=false，其中 30 人
    // 已背着不该有的发件副本。写值改由本人这一列决定。
    let keep_copy: bool = sqlx::query_scalar(
        "SELECT save_pm FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let id: i64 = sqlx::query_scalar(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, location, saved, unread) \
         VALUES ($1, $2, $3, $4, 1, $5, true) RETURNING id",
    )
    .bind(auth.id)
    .bind(to_id)
    .bind(&subject)
    .bind(&text)
    .bind(if keep_copy { 1 } else { 0 })
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
    // Web Push（0283 P0-1）：私信即时推送——后台尽力而为，不阻塞响应。
    // 收信人 notice_prefs.push_message 为 false 时由 push_to_user 内部过滤。
    {
        let (db, to, subj, txt) =
            (state.repo.db.clone(), to_id, subject.clone(), text.clone());
        let sender: String =
            sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
                .bind(auth.id)
                .fetch_optional(&state.repo.db)
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
        actix_web::rt::spawn(async move {
            crate::push_notify::push_to_user(
                &db,
                to,
                "message",
                &format!("来自 {sender} 的私信"),
                &format!("{subj}\n\n{txt}"),
            )
            .await;
        });
    }
    Ok(ok(serde_json::json!({ "id": id })))
}
