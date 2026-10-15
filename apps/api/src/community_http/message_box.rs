//! M16 信箱操作（已读/删除/移动/文件夹）。
//! 从 community_http.rs 按域拆出。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 信箱操作（messages.php 口径：已读/逻辑删/移动/文件夹） ============

#[derive(Deserialize, utoipa::ToSchema)]
struct MarkReadReq {
    ids: Vec<i64>,
}

/// 标记已读（单条打开或列表批量 markread）
#[utoipa::path(
    post,
    path = "/api/v1/messages/markread",
    tag = "社区/消息",
    description = "标记已读（可按 id 列表或整信箱）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/messages/markread")]
pub(crate) async fn message_markread(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MarkReadReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    let n = sqlx::query(
        "UPDATE messages SET unread = false, read_at = COALESCE(read_at, now()) \
         WHERE receiver_id = $1 AND id = ANY($2)",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "updated": n })))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct DeleteReq {
    ids: Vec<i64>,
}

/// 逻辑删除（NexusPHP 双删语义）：收件方删 → location=0；发件方删 → saved=0；
/// 两边都删才物理 DELETE。
#[utoipa::path(
    post,
    path = "/api/v1/messages/delete",
    tag = "社区/消息",
    description = "删站内信（移到回收站；location 字段区分）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/messages/delete")]
pub(crate) async fn message_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<DeleteReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // P-297：先数「本次真的会动到的行」。此前响应回的是 body.ids.len()
    // （请求条数）——不存在的 id、别人的 id、已被对方删掉的 id 全都算成
    // 「已删除」：现网实测拿 3 个不存在的 id 打过来回 {"deleted": 3}，
    // 而同口的 markread / move 对同一个 id 都老实回 0。用户按这个数字
    // 判断「删掉了没有」，而未读数照旧 ⇒ 「返回值当事实」那一族的又一次。
    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM messages WHERE id = ANY($1) AND \
         (receiver_id = $2 OR sender_id = $2)",
    )
    .bind(&body.ids)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE messages SET location = 0 WHERE receiver_id = $1 AND \
         id = ANY($2) AND location = 1",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE messages SET saved = 0 WHERE sender_id = $1 AND id = \
         ANY($2) AND saved = 1",
    )
    .bind(auth.id)
    .bind(&body.ids)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 归属条件不能省（P-340）。前面两条 UPDATE 都带 receiver_id /
    // sender_id，而这条物理删除只按 `id = ANY($1) AND location = 0 AND
    // saved = 0`。它本意是“把本人删完后剩下的回收站行一并清掉”，
    // 但只要那行**不属于本人**（修正前产品里存在这种态：别人的信被删过
    // 一次、location 落到 0，saved=0），任何人对着那个 id 调本口都能把它物理删掉。
    // 实测：造一封 receiver=6311 且已在回收站的信，root 调 POST /messages/delete
    // → 回应 {deleted: 0}（声称一检未删），而那行**已经没了**。
    // 归属条件与前两条同口径。
    sqlx::query(
        "DELETE FROM messages WHERE id = ANY($1) AND location = 0 \
     AND saved = 0 AND (receiver_id = $2 OR sender_id = $2)",
    )
    .bind(&body.ids)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "deleted": owned })))
}

/// `deny_unknown_fields` 是必需的：读口的查询参数叫 `box_id`，写口这个字段叫
/// `folder`——两个名字指同一件事。拼错键名时 serde 默认忽略未知键并把
/// `folder` 当 `None`，而 `folder=NULL` 的语义是「移回主收件箱」：请求会
/// **反向改数据并回 200 moved=1**（2026-10-15 实测：`{ids, box_id}` 发出去，
/// 角标与主夹未读列表一动不动，接口却报成功）。
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct MoveReq {
    ids: Vec<i64>,
    folder: Option<i32>,
}

/// 移动到自建文件夹（folder=NULL 即移回主收件箱）
#[utoipa::path(
    post,
    path = "/api/v1/messages/move",
    tag = "社区/消息",
    description = "移动站内信到自定义文件夹。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/messages/move")]
pub(crate) async fn message_move(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MoveReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.ids.is_empty() {
        return Err(DomainError::Validation("未选择信件".into()));
    }
    if let Some(f) = body.folder {
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM pmboxes WHERE id = $1 AND \
             user_id = $2)",
        )
        .bind(f)
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(false);
        if !owned {
            return Err(DomainError::Validation("目标文件夹不存在".into()));
        }
    }
    let n = sqlx::query(
        "UPDATE messages SET folder = $2 WHERE receiver_id = $1 AND id \
         = ANY($3) AND location = 1",
    )
    .bind(auth.id)
    .bind(body.folder)
    .bind(&body.ids)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    Ok(ok(serde_json::json!({ "moved": n })))
}

/// 文件夹列表 + 新建/改名（editmailboxes 口径：一人最多 3 个、名 ≤14 字；清空名=删除并连带清信）
#[cfg(test)]
mod tests {
    use super::MoveReq;

    /// P-310：读口参数叫 `box_id`、写口字段叫 `folder`，拼错键名必须被拒——
    /// 否则 serde 忽略未知键、`folder` 落 None，等于把信静默移回主收件箱还回
    /// 200（2026-10-15 实测过的那一刻：badge 与列表一动不动，moved=1）。
    #[test]
    fn move_rejects_mistyped_field_names() {
        let bad: Result<MoveReq, _> =
            serde_json::from_str(r#"{"ids":[1],"box_id":2}"#);
        assert!(bad.is_err(), "未知键 box_id 必须被拒");
        let e = bad.unwrap_err().to_string();
        assert!(e.contains("unknown field"), "报错要指出是哪个键：{e}");
        let good: Result<MoveReq, _> =
            serde_json::from_str(r#"{"ids":[1],"folder":2}"#);
        assert!(good.is_ok(), "正控：正确字段名必须收");
        let back: Result<MoveReq, _> =
            serde_json::from_str(r#"{"ids":[1],"folder":null}"#);
        assert!(back.is_ok(), "移回主收件箱（folder=null）仍要收");
    }
}
