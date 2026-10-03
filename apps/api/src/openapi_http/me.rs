//! 开放 API：本人账号态只读（0268）。
//!
//! 补「用户侧数据」缺口：此前 Token 只能拿到账号汇总（uploaded/downloaded），
//! H&R 违约与站内信对外一个都没有 —— 移动壳（pt_mate 类）只能去爬 HTML。
//! 全部只读、只暴露 Token 主人自己的数据（uid 一律来自 Token，不收路径参数）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::{require_scope, require_token, with_rl, SCOPE_READ};

/// 单次返回上限
pub(super) const MAX_ROWS: i64 = 100;

/// GET /open/me/overview —— 一张面板聚合（自动化工具轮询这一条就够）。
#[get("/open/me/overview")]
pub(super) async fn me_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let uid = tk.uid;
    // 注意 class_id 是 i32，其余为 bigint —— 元组类型必须逐位对齐，否则 sqlx 报 500
    let row: Option<(i64, i64, i64, i64, i64, i64, i64, i32)> = sqlx::query_as(
        "SELECT u.uploaded, u.downloaded, COALESCE(u.spark_balance,0), \
         (SELECT count(*) FROM snatches s WHERE s.user_id=u.id AND s.seeding), \
         (SELECT count(*) FROM snatches s WHERE s.user_id=u.id AND s.leeching), \
         (SELECT count(*) FROM messages m WHERE m.receiver_id=u.id \
             AND m.read_at IS NULL), \
         (SELECT count(*) FROM hr_violations h WHERE h.user_id=u.id \
             AND h.resolved_at IS NULL), \
         u.class_id \
         FROM users u WHERE u.id = $1 AND u.status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((up, down, bonus, seeding, leeching, unread, hr_open, class_id)) =
        row
    else {
        return Err(DomainError::Unauthorized);
    };
    let ratio = if down > 0 {
        (up as f64 / down as f64 * 10000.0).round() / 10000.0
    } else {
        0.0
    };
    Ok(with_rl(
        ok(serde_json::json!({
            "uploaded": up,
            "downloaded": down,
            "ratio": ratio,
            "bonus": bonus,
            "seeding": seeding,
            "leeching": leeching,
            "unread_messages": unread,
            "hr_open": hr_open,
            "class_id": class_id,
            "hint": "细项见 /open/me/seeding | /open/me/history | /open/me/hr | /open/me/messages",
        })),
        &tk,
    ))
}

/// GET /open/me/hr?status=open —— H&R 违约清单（open=未解决，all=全部）。
#[get("/open/me/hr")]
pub(super) async fn me_hr(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let status = q.get("status").map(String::as_str).unwrap_or("open");
    if !matches!(status, "open" | "all") {
        return Err(DomainError::Validation("status 只认 open / all".into()));
    }
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, MAX_ROWS);
    let sql = format!(
        "SELECT h.torrent_id, t.name, h.seeded_seconds, h.required_seconds, \
         EXTRACT(EPOCH FROM h.detected_at)::bigint, \
         EXTRACT(EPOCH FROM h.resolved_at)::bigint \
         FROM hr_violations h JOIN torrents t ON t.id = h.torrent_id \
         WHERE h.user_id = $1 {} ORDER BY h.id DESC LIMIT $2",
        if status == "open" {
            "AND h.resolved_at IS NULL"
        } else {
            ""
        },
    );
    let rows: Vec<(i64, String, i32, i32, i64, Option<i64>)> =
        sqlx::query_as(&sql)
            .bind(tk.uid)
            .bind(limit)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(tid, name, seeded, need, det, res)| {
            serde_json::json!({
                "torrent_id": tid,
                "name": name,
                "seeded_seconds": seeded,
                "required_seconds": need,
                "shortfall_seconds": (need - seeded).max(0),
                "detected_at": det,
                "resolved_at": res,
                "open": res.is_none(),
            })
        })
        .collect();
    Ok(with_rl(
        ok(serde_json::json!({ "items": items, "count": items.len() })),
        &tk,
    ))
}

/// GET /open/me/messages?unread=1&limit=50 —— 站内信（本人）。
#[get("/open/me/messages")]
pub(super) async fn me_messages(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let unread_only = matches!(
        q.get("unread").map(String::as_str),
        Some("1") | Some("true")
    );
    let limit = q
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(50)
        .clamp(1, MAX_ROWS);
    let sql = format!(
        "SELECT m.id, COALESCE(su.username, '系统'), m.subject, m.body, \
         EXTRACT(EPOCH FROM m.created_at)::bigint, \
         EXTRACT(EPOCH FROM m.read_at)::bigint \
         FROM messages m LEFT JOIN users su ON su.id = m.sender_id \
         WHERE m.receiver_id = $1 {} ORDER BY m.id DESC LIMIT $2",
        if unread_only {
            "AND m.read_at IS NULL"
        } else {
            ""
        },
    );
    let rows: Vec<(i64, String, String, String, i64, Option<i64>)> =
        sqlx::query_as(&sql)
            .bind(tk.uid)
            .bind(limit)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, from, subject, body, at, read_at)| {
            serde_json::json!({
                "id": id, "from": from, "subject": subject, "body": body,
                "created_at": at, "read_at": read_at, "unread": read_at.is_none(),
            })
        })
        .collect();
    Ok(with_rl(
        ok(serde_json::json!({ "items": items, "count": items.len() })),
        &tk,
    ))
}
