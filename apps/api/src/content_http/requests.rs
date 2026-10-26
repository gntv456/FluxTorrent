use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::earn_spark_tx;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

mod bid;
mod create;
pub(crate) use bid::request_bid;
pub(crate) use create::request_create;

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

/// 求种列表（同类站 viewrequests.php 口径）：finished 筛选 + 名称搜索。
/// 最新出价/加价人次自 0417 起为真数据（request_bids 明细表）。
#[get("/requests")]
pub(super) async fn request_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
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
    // 0417：latest_bounty/bids 从 request_bids 明细聚合（不再兜底伪数据）。
    // LATERAL 走 request_bids_req_idx（request_id, id DESC），每行恒定两次
    // 索引探测，无 GROUP BY 全表物化。
    let cols = "r.id, u.username, r.title, r.descr, r.bounty, r.status, \
                r.fulfilled_torrent_id, r.created_at, \
                lb.amount, bc.cnt";
    let laterals = "\
         LEFT JOIN LATERAL ( \
           SELECT amount FROM request_bids b \
           WHERE b.request_id = r.id ORDER BY b.id DESC LIMIT 1 \
         ) lb ON TRUE \
         LEFT JOIN LATERAL ( \
           SELECT count(*) AS cnt FROM request_bids b \
           WHERE b.request_id = r.id \
         ) bc ON TRUE";
    let rows: Vec<serde_json::Value> = if only_mine {
        sqlx::query_as::<_, (i64, Option<String>, String, Option<String>, i64, i16, Option<i64>, chrono::DateTime<chrono::Utc>, Option<i64>, i64)>(
            &format!(
                "SELECT {cols} \
                 FROM requests r LEFT JOIN users u ON u.id = r.user_id \
                 {laterals} \
                 WHERE r.user_id = $2 AND r.title ILIKE $1 \
                 ORDER BY r.id DESC LIMIT 50",
            ),
        )
        .bind(&search)
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
    } else {
        sqlx::query_as::<_, (i64, Option<String>, String, Option<String>, i64, i16, Option<i64>, chrono::DateTime<chrono::Utc>, Option<i64>, i64)>(
            &format!(
                "SELECT {cols} \
                 FROM requests r LEFT JOIN users u ON u.id = r.user_id \
                 {laterals} \
                 WHERE {status_cond} AND r.title ILIKE $1 \
                 ORDER BY r.id DESC LIMIT 50",
            ),
        )
        .bind(&search)
        .fetch_all(&state.repo.db)
        .await
    }
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, username, title, descr, bounty, status, fulfilled, ts, latest_bid, bid_cnt)| {
        serde_json::json!({
            "id": id, "username": username, "title": title, "descr": descr,
            "bounty": bounty, "latest_bounty": latest_bid,
            // 0417 前 latest_bounty/bids 是兜底伪数据；现在为真聚合。
            // comments 仍为 null：求种单没有评论线，伪装成 0 会读成「零条讨论」。
            "comments": null, "bids": bid_cnt,
            "status": status, "fulfilled_torrent_id": fulfilled, "created_at": ts,
        })
    })
    .collect();
    Ok(ok(rows))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
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
    let r: Option<(i64, i64, i64, i16, String)> = sqlx::query_as(
        "SELECT id, user_id, bounty, status, title FROM requests WHERE id = $1",
    )
    .bind(body.request_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, requester, bounty, status, title)) = r else {
        return Err(DomainError::NotFound(body.request_id));
    };
    if status != 0 {
        return Err(DomainError::Validation("该求种已处理".into()));
    }
    let t_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND \
         approval_status = 1)",
    )
    .bind(body.torrent_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !t_exists {
        return Err(DomainError::TorrentInvalid("种子不存在或未过审".into()));
    }
    // 应种人必须是该种子的发布者（旧站口径）：防止他人拿别人的 torrent_id
    // 完结求种、把悬赏转入自己账户（原实现任何登录用户可领任意求种的 bounty）。
    // 0417 放宽一档：staff（class≥90）可替任何种子应种——「楼主发完种离站、
    // 无人能交付」的僵局此前只能进库改数据。staff 冒领的审计面由 audit_log
    // 不变式覆盖（写口全程留痕），经济面无增量风险（悬赏仍全额归发布者）。
    let is_staff = auth.class_id >= 90;
    let owner_match: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND owner_id = $2)",
    )
    .bind(body.torrent_id)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !owner_match && !is_staff {
        return Err(DomainError::Validation(
            "只有该种子的发布者可以应种此求种（管理组可代为交付）".into(),
        ));
    }
    // staff 代交付时悬赏必须归发布者而不是自己：payee 换成 torrent.owner_id。
    let torrent_owner: i64 =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(body.torrent_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 完结与发奖必须同事务（批十九 P-26）：旧实现把「UPDATE status=1」与
    // 「earn_spark 转悬赏」拆成两条 autocommit。发钱一失败（余额溢出、
    // 流水冲突、瞬断）请求已经是「完结」，而入口守卫 `status != 0` 又把
    // 重试路径堵死——求种人被冻结的悬赏既回不到他账上、也到不了应种人手里，
    // 后台没有任何补救口。
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let updated = sqlx::query(
        "UPDATE requests SET status = 1, \
         fulfilled_torrent_id = $2 WHERE id = $1 AND status = 0",
    )
    .bind(id)
    .bind(body.torrent_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::LedgerConflict);
    }
    // 悬赏：发布者得（含 staff 代交付——归 torrent_owner 而非经办 staff）；
    // 楼主自应自求 → 退还。payee 与上面 owner 校验/torrent_owner 查询同源。
    let mut pm_bounty = 0i64;
    if bounty > 0 {
        let payee = if requester != torrent_owner {
            torrent_owner
        } else {
            requester
        };
        let idem = format!("req-payout:{}", id);
        // Replayed = 这笔悬赏早就入账过；那说明有上一次尝试半途而废，
        // 此时不能把「完结」落库（会再次掩盖问题），显式冲突回滚。
        if !matches!(
            earn_spark_tx(&mut tx, payee, bounty, "task_reward", &idem).await?,
            crate::economy_http::SpendOutcome::Spent
        ) {
            return Err(DomainError::LedgerConflict);
        }
        pm_bounty = bounty;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 求种人应当被通知（旧实现发完钱就走，求种人只能自己回列表翻）。
    // 站内信按 0286 口径带 kind/params 并过 u_notice_enabled；写入失败只
    // 出声不阻断——完结已落库，因通知失败回 500 会让人重复应求。
    if requester != torrent_owner {
        let pm = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body, \
                kind, params) \
             SELECT NULL, $1, '你求的种已被应求', \
                format('求种 #%s「%s」已被应求，关联种子 #%s，悬赏 %s 魔力已转给应种人。', \
                       $2::text, $3, $4::text, $5), \
                'request_fulfilled', \
                jsonb_build_object('request_id', $2, 'title', $3, \
                                   'torrent_id', $4, 'bounty', $5) \
             WHERE u_notice_enabled($1, 'request_fulfilled')",
        )
        .bind(requester)
        .bind(id)
        .bind(&title)
        .bind(body.torrent_id)
        .bind(pm_bounty)
        .execute(&state.repo.db)
        .await;
        if let Err(e) = pm {
            tracing::warn!(?e, request = id, "求种应求通知写入失败");
        }
    }
    Ok(ok(
        serde_json::json!({ "request_id": id, "bounty_paid": bounty }),
    ))
}
