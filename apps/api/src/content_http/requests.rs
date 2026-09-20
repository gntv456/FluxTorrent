use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::economy_http::spend_spark_tx;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[post("/requests")]
pub(super) async fn request_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RequestCreateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("求种标题不能为空".into()));
    }
    if body.bounty < 0 {
        return Err(DomainError::Validation("悬赏不能为负".into()));
    }
    // 悬赏即时冻结（从余额划走，应种交付时转移给应种人）。
    // 审计修复（P0 吞钱）：扣费与建单此前非原子——INSERT 失败（超长/瞬断）时扣款已提交、
    // 且幂等键带随机 UUID 不可追回。改为：先建单占坑 → 扣费失败连单回滚（同一事务语义）。
    let id: i64 = if body.bounty > 0 {
        let mut tx = state
            .repo
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO requests (user_id, title, descr, bounty) VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(auth.id)
        .bind(&body.title)
        .bind(&body.descr)
        .bind(body.bounty)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // spend_spark 自带事务（锁行→幂等→扣款→流水）；此处复用同一连接保证原子性。
        // 幂等键含新建单 id，重放不可达；闸门为 #[must_use] 契约统一口径。
        let idem = format!("req-bounty:{}:{}", auth.id, id);
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
            "INSERT INTO requests (user_id, title, descr, bounty) VALUES ($1, $2, $3, $4) RETURNING id",
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

#[derive(sqlx::FromRow, serde::Serialize)]
#[allow(dead_code)] // 列表响应复用字段，部分列暂未在 handler 中读取
struct RequestRow {
    id: i64,
    username: Option<String>,
    title: String,
    descr: Option<String>,
    bounty: i64,
    status: i16,
    fulfilled_torrent_id: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 求种列表（包子站 viewrequests.php 口径）：finished 筛选 + 名称搜索。
/// 最新出价/评论数/应求数：数据模型暂无加价与评论表，取 bounty/0/0 兜底。
#[get("/requests")]
pub(super) async fn request_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let finished = q.get("finished").map(|s| s.as_str()).unwrap_or("no");
    let search = q
        .get("search")
        .map(|s| crate::http::like_pattern(&s))
        .unwrap_or_else(|| "%".into());
    let status_cond = match finished {
        "all" => "TRUE",
        "yes" => "r.status = 1",
        "ing" => "r.status = 2",
        _ => "r.status = 0",
    };
    let only_mine = finished == "my";
    let rows: Vec<serde_json::Value> = if only_mine {
        sqlx::query_as::<_, (i64, Option<String>, String, Option<String>, i64, i16, Option<i64>, chrono::DateTime<chrono::Utc>)>(
            "SELECT r.id, u.username, r.title, r.descr, r.bounty, r.status, r.fulfilled_torrent_id, r.created_at \
             FROM requests r LEFT JOIN users u ON u.id = r.user_id \
             WHERE r.user_id = $2 AND r.title ILIKE $1 ORDER BY r.id DESC LIMIT 50",
        )
        .bind(&search)
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
    } else {
        sqlx::query_as::<_, (i64, Option<String>, String, Option<String>, i64, i16, Option<i64>, chrono::DateTime<chrono::Utc>)>(
            &format!(
                "SELECT r.id, u.username, r.title, r.descr, r.bounty, r.status, r.fulfilled_torrent_id, r.created_at \
                 FROM requests r LEFT JOIN users u ON u.id = r.user_id \
                 WHERE {status_cond} AND r.title ILIKE $1 ORDER BY r.id DESC LIMIT 50",
            ),
        )
        .bind(&search)
        .fetch_all(&state.repo.db)
        .await
    }
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, username, title, descr, bounty, status, fulfilled, ts)| {
        serde_json::json!({
            "id": id, "username": username, "title": title, "descr": descr,
            "bounty": bounty, "latest_bounty": bounty,
            "comments": 0, "bids": 0,
            "status": status, "fulfilled_torrent_id": fulfilled, "created_at": ts,
        })
    })
    .collect();
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FulfillReq {
    request_id: i64,
    torrent_id: i64,
}

#[post("/requests/fulfill")]
pub(super) async fn request_fulfill(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FulfillReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let r: Option<(i64, i64, i64, i16)> = sqlx::query_as(
        "SELECT id, user_id, bounty, status FROM requests WHERE id = $1",
    )
    .bind(body.request_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, requester, bounty, status)) = r else {
        return Err(DomainError::NotFound(body.request_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该求种已处理".into()));
    }
    let t_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND approval_status = 1)",
    )
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !t_exists {
        return Err(DomainError::TorrentInvalid("种子不存在或未过审".into()));
    }
    // 应种人必须是该种子的发布者（旧站口径）：防止他人拿别人的 torrent_id
    // 完结求种、把悬赏转入自己账户（原实现任何登录用户可领任意求种的 bounty）
    let owner_match: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND owner_id = $2)",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !owner_match {
        return Err(DomainError::Validation(
            "只有该种子的发布者可以应种此求种".into(),
        ));
    }
    let updated = sqlx::query(
        "UPDATE requests SET status = 1, fulfilled_torrent_id = $2 WHERE id = $1 AND status = 0",
    )
    .bind(id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 悬赏：应种给他人 → 转移；自己应自己的 → 退还
    if bounty > 0 {
        let payee = if requester != auth.id {
            auth.id
        } else {
            requester
        };
        let idem = format!("req-payout:{}", id);
        earn_spark(&state.repo.db, payee, bounty, "task_reward", &idem).await?;
    }
    Ok(ok(
        serde_json::json!({ "request_id": id, "bounty_paid": bounty }),
    ))
}

// ============ M17 候选 ============

// ============ M17 求种 ============

#[derive(Deserialize)]
struct RequestCreateReq {
    title: String,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    bounty: i64,
}
