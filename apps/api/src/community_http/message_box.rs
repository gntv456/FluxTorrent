//! M16 信箱操作（已读/删除/移动/文件夹）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 信箱操作（messages.php 口径：已读/逻辑删/移动/文件夹） ============

#[derive(Deserialize)]
struct MarkReadReq {
    ids: Vec<i64>,
}

/// 标记已读（单条打开或列表批量 markread）
#[post("/messages/markread")]
async fn message_markread(
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

#[derive(Deserialize)]
struct DeleteReq {
    ids: Vec<i64>,
}

/// 逻辑删除（NexusPHP 双删语义）：收件方删 → location=0；发件方删 → saved=0；
/// 两边都删才物理 DELETE。
#[post("/messages/delete")]
async fn message_delete(
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
    sqlx::query(
        "DELETE FROM messages WHERE id = ANY($1) AND location = 0 \
     AND saved = 0",
    )
    .bind(&body.ids)
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
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MoveReq {
    ids: Vec<i64>,
    folder: Option<i32>,
}

/// 移动到自建文件夹（folder=NULL 即移回主收件箱）
#[post("/messages/move")]
async fn message_move(
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
#[derive(sqlx::FromRow, serde::Serialize)]
struct PmBoxRow {
    id: i64,
    name: String,
    count: i64,
}

#[get("/messages/boxes")]
async fn message_boxes(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, PmBoxRow>(
        "SELECT b.id, b.name, (SELECT count(*) FROM messages m WHERE m.folder = b.id AND m.receiver_id = $1 AND m.location = 1) AS count \
         FROM pmboxes b WHERE b.user_id = $1 ORDER BY b.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct BoxReq {
    /// 缺省 = 新建；有值 = 改名/删除该 id
    id: Option<i64>,
    name: String,
}

#[post("/messages/boxes")]
async fn message_box_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BoxReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let name = body.name.trim();
    // 清空名 = 删除文件夹（连带清掉夹内信件，NexusPHP 口径）
    if name.is_empty() {
        if let Some(bid) = body.id {
            sqlx::query(
                "DELETE FROM messages WHERE folder = $2 AND receiver_id = $1",
            )
            .bind(auth.id)
            .bind(bid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query("DELETE FROM pmboxes WHERE id = $2 AND user_id = $1")
                .bind(auth.id)
                .bind(bid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        return Ok(ok(serde_json::json!({ "deleted": true })));
    }
    if name.chars().count() > 14 {
        return Err(DomainError::Validation("文件夹名最多 14 字".into()));
    }
    match body.id {
        None => {
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pmboxes WHERE user_id = $1",
            )
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
            if n >= 3 {
                return Err(DomainError::Validation("最多 3 个文件夹".into()));
            }
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO pmboxes (user_id, name) VALUES ($1, $2) \
                 ON CONFLICT (user_id, name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
            )
            .bind(auth.id)
            .bind(name)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "id": id })))
        }
        Some(bid) => {
            let n = sqlx::query(
                "UPDATE pmboxes SET name = $3 WHERE id = $2 AND user_id = $1",
            )
            .bind(auth.id)
            .bind(bid)
            .bind(name)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
            if n == 0 {
                return Err(DomainError::NotFound(0));
            }
            Ok(ok(serde_json::json!({ "renamed": true })))
        }
    }
}

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
