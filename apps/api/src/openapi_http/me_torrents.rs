//! 开放 API：本人种子活动只读（0268）—— 做种列表与下载历史。
//!
//! 数据源是 snatches（抓取表，含本人 uploaded/downloaded/做种时长/进度），
//! 关联 torrents 取标题与做种数。移动壳的「我的做种 / 我的下载」直接用这两条。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use crate::torrents::CatMap;

use super::paging::{count_with, has_more, page_params};
use super::{require_scope, require_token, with_rl, SCOPE_READ};

/// 列表与计数共用的 FROM/WHERE 片段（`$1` = Token 主人 uid）。
/// P-451：`total` 数的必须正是「能翻出来的那些行」——两处各写一份
/// 迟早分叉，所以这里只留一份给两边共用（同仓 `/me/torrentlist` 同口径）。
fn snatch_fw(seeding_only: bool) -> String {
    format!(
        "snatches s JOIN torrents t ON t.id = s.torrent_id \
         WHERE s.user_id = $1{}",
        if seeding_only {
            " AND s.seeding"
        } else {
            ""
        }
    )
}

/// snatches × torrents 的共同投影（做种/历史两个端点同一形状，工具只需认一次）
#[allow(clippy::type_complexity)]
async fn my_snatches(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    seeding_only: bool,
    limit: i64,
    offset: i64,
) -> DomainResult<
    Vec<(
        i64,
        String,
        i64,
        i32,
        i32,
        i32,
        i64,
        i64,
        Option<i64>,
        Option<i64>,
        i32,
        bool,
        bool,
    )>,
> {
    let fw = snatch_fw(seeding_only);
    let sql = format!(
        "SELECT t.id, t.name, t.size, t.seeders, t.leechers, t.category_id, \
         s.uploaded, s.downloaded, \
         EXTRACT(EPOCH FROM s.completed_at)::bigint, \
         EXTRACT(EPOCH FROM s.last_seen_at)::bigint, \
         s.progress, s.seeding, s.leeching \
         FROM {} ORDER BY s.last_seen_at DESC NULLS LAST, \
         s.torrent_id DESC LIMIT $2 OFFSET $3",
        fw
    );
    sqlx::query_as(&sql)
        .bind(uid)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}

fn project(
    cats: &CatMap,
    tid: i64,
    name: &str,
    size: i64,
    seeders: i32,
    leechers: i32,
    cat: i32,
    my_up: i64,
    my_down: i64,
    completed: Option<i64>,
    seen: Option<i64>,
    progress: i32,
    seeding: bool,
    leeching: bool,
) -> serde_json::Value {
    serde_json::json!({
        "torrent_id": tid,
        "name": name,
        "size": size,
        "seeders": seeders,
        "leechers": leechers,
        "category": cat,
        "category_name": cats.name(cat),
        "my_uploaded": my_up,
        "my_downloaded": my_down,
        "progress_percent": (progress / 100).clamp(0, 100),
        "completed_at": completed,
        "last_seen_at": seen,
        "seeding": seeding,
        "leeching": leeching,
    })
}

/// GET /open/me/seeding?limit=50 —— 我正在做种的清单（含本人收益口径）。
#[utoipa::path(
    get,
    path = "/api/v1/open/me/seeding",
    tag = "我的数据",
    description = "我正在做种的清单（含 my_uploaded 与进度）。",
    params((
        "limit" = String,
        Query,
        description = "1-100，默认 50"
    ),
    ("offset" = String, Query, description = "跳过前 N 行，默认 0")),
    responses(
        (status = 200, description = "items + count"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 403, description = "缺所需 scope（code 2003）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/open/me/seeding")]
pub(super) async fn me_seeding(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let (limit, offset) = page_params(&q);
    let cats = CatMap::load(&state.repo.db).await;
    let rows = my_snatches(&state, tk.uid, true, limit, offset).await?;
    let total = count_with(&state, tk.uid, &snatch_fw(true)).await?;
    let items: Vec<_> = rows
        .into_iter()
        .map(
            |(
                tid,
                name,
                size,
                sd,
                lc,
                cat,
                up,
                down,
                done,
                seen,
                prog,
                _,
                _,
            )| {
                project(
                    &cats, tid, &name, size, sd, lc, cat, up, down, done, seen,
                    prog, true, false,
                )
            },
        )
        .collect();
    let got = items.len() as i64;
    Ok(with_rl(
        ok(serde_json::json!({
            "items": items,
            // count = 本次返回条数；total = 这套谓词下的总行数；
            // has_more = 后面还有（P-451 前这两个字段根本不存在，
            // 客户端只能拿到被 limit 截断的一页还当它是全部）
            "count": got,
            "total": total,
            "offset": offset,
            "limit": limit,
            "has_more": has_more(offset, got, total),
        })),
        &tk,
    ))
}

/// GET /open/me/history?limit=50 —— 我的下载/抓取历史（按最近活跃排序）。
#[utoipa::path(
    get,
    path = "/api/v1/open/me/history",
    tag = "我的数据",
    description = "我的下载/抓取历史（按最近活跃排序）。",
    params((
        "limit" = String,
        Query,
        description = "1-100，默认 50"
    ),
    ("offset" = String, Query, description = "跳过前 N 行，默认 0")),
    responses(
        (status = 200, description = "items + count"),
        (status = 401, description = "Token 无效/已撤销/未传（code 2001）"),
        (status = 403, description = "缺所需 scope（code 2003）"),
        (status = 429, description = "限流（code 1015），带 Retry-After: 60"),
    )
)]
#[get("/open/me/history")]
pub(super) async fn me_history(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let (limit, offset) = page_params(&q);
    let cats = CatMap::load(&state.repo.db).await;
    let rows = my_snatches(&state, tk.uid, false, limit, offset).await?;
    let total = count_with(&state, tk.uid, &snatch_fw(false)).await?;
    let items: Vec<_> = rows
        .into_iter()
        .map(
            |(
                tid,
                name,
                size,
                sd,
                lc,
                cat,
                up,
                down,
                done,
                seen,
                prog,
                sd2,
                lc2,
            )| {
                project(
                    &cats, tid, &name, size, sd, lc, cat, up, down, done, seen,
                    prog, sd2, lc2,
                )
            },
        )
        .collect();
    let got = items.len() as i64;
    Ok(with_rl(
        ok(serde_json::json!({
            "items": items,
            // count = 本次返回条数；total = 这套谓词下的总行数；
            // has_more = 后面还有（P-451 前这两个字段根本不存在，
            // 客户端只能拿到被 limit 截断的一页还当它是全部）
            "count": got,
            "total": total,
            "offset": offset,
            "limit": limit,
            "has_more": has_more(offset, got, total),
        })),
        &tk,
    ))
}
