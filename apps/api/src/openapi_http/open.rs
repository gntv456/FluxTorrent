//! 开放数据接口（Token 鉴权）与 OpenAPI 文档（与实现同步维护）。
//! 从 openapi_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use crate::torrents::{CatMap, MediaMap};

use super::{require_scope, require_token, with_rl, SCOPE_READ};

/// 单次返回上限（recent / announces 共用）
const MAX_ROWS: i64 = 200;

#[derive(serde::Deserialize)]
pub struct OpenQuery {
    #[serde(default)]
    limit: Option<i64>,
    /// 增量游标（announces 用）：只返回 id 严格大于它的种子
    #[serde(default)]
    since_id: Option<i64>,
    /// 增量游标（按发布时间，Unix 秒）：与 since_id 叠加生效
    #[serde(default)]
    since: Option<i64>,
}

/// 种子行的对外投影（recent / announces 共用同一形状，工具只需认一次）。
#[allow(clippy::too_many_arguments)]
fn project(
    id: i64,
    info_hash: String,
    pieces_hash: Option<String>,
    name: String,
    descr: Option<String>,
    size: i64,
    ts: i64,
    seeders: i32,
    leechers: i32,
    category: i32,
    cats: &CatMap,
    dims: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "name": name,
        "description": descr,
        "size": size,
        "published_at": ts,
        "seeders": seeders,
        "leechers": leechers,
        "category": category,
        "category_name": cats.name(category),
        "category_np": cats.legacy(category),
        "category_newznab": cats.newznab(category),
        // 0267：辅种/查重工具需要的双指纹（此前一个都不给，
        // 文档却建议「按 pieces_hash 查重」——承诺大于能力，工具照做必踩空）
        "info_hash": info_hash,
        "pieces_hash": pieces_hash,
        // 分型字段（批次 3 尾巴/0329）：label→首值的扁平摘要
        // （季/话数/作者/联赛…按站型），列表工具不用再逐条打详情
        "dimensions": dims,
    })
}

/// 批量取一组种子的维度摘要（label→首值）。列表投影用：
/// 一条 SQL 覆盖整页，避免 N+1。
#[allow(clippy::type_complexity)]
async fn dims_for(
    db: &sqlx::PgPool,
    ids: &[i64],
) -> std::collections::HashMap<i64, serde_json::Value> {
    let mut out: std::collections::HashMap<i64, serde_json::Value> =
        std::collections::HashMap::new();
    if ids.is_empty() {
        return out;
    }
    let rows: Vec<(i64, String, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT ts.torrent_id, COALESCE(k.label, ts.kind),              d.name, ts.value #>> '{}'              FROM torrent_sections ts              LEFT JOIN section_dict d ON d.id = ts.dict_id              LEFT JOIN section_kinds k ON k.kind = ts.kind              WHERE ts.torrent_id = ANY($1)              ORDER BY COALESCE(k.sort, 999), ts.ordinal",
        )
        .bind(ids)
        .fetch_all(db)
        .await
        .unwrap_or_default();
    for (tid, label, dict_name, free) in rows {
        let v = dict_name.or(free);
        if let Some(v) = v {
            let e = out
                .entry(tid)
                .or_insert_with(|| serde_json::Map::new().into());
            if let Some(m) = e.as_object_mut() {
                // 同 label 多值保首个（列表摘要语义；全值走详情 sections）
                m.entry(label).or_insert(serde_json::json!(v));
            }
        }
    }
    out
}

/// 开放接口：最新种子（RSS 增强客户端/移动壳可用的稳定只读契约）。
///
/// 0267 修正：此前当 `guest_policy = all_private`（默认）时**直接返回空数组**。
/// 但本端点要求有效 API Token —— 调用者已是本站注册用户，游客策略管的是
/// 「未登录可见度」，不该拦已认证成员（文档承诺「拿最新 50 条做批量反查」，
/// 默认配置下却恒空，工具无从判断是「站里没种」还是「被策略挡了」）。
#[get("/open/recent")]
pub(super) async fn open_recent_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<OpenQuery>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, MAX_ROWS);
    let cats = CatMap::load(&state.repo.db).await;
    type Row = (
        i64,
        String,
        Option<String>,
        String,
        Option<String>,
        i64,
        i64,
        i32,
        i32,
        i32,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, info_hash, pieces_hash, name, small_descr, size, \
         EXTRACT(EPOCH FROM created_at)::bigint, seeders, leechers, \
         category_id \
         FROM torrents WHERE approval_status = 1 \
         ORDER BY id DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let dims = dims_for(
        &state.repo.db,
        &rows.iter().map(|r| r.0).collect::<Vec<_>>(),
    )
    .await;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, ih, ph, name, descr, size, ts, sd, lc, cat)| {
            let d = dims.get(&id).cloned().unwrap_or(
                serde_json::Map::new().into(),
            );
            project(id, ih, ph, name, descr, size, ts, sd, lc, cat, &cats, d)
        })
        .collect();
    Ok(with_rl(ok(items), &tk))
}

/// 开放接口：增量新种流（0267，autobrr / 自建推送器用）。
///
/// 语义：返回 `id > since_id`（且发布时间 ≥ `since`）的过审种子，按 id 升序。
/// 调用方保存响应里的 `next_since_id` 作为下次游标，即可做「只推新种」的轮询，
/// 无需反复拉全量列表。无匹配时返回空数组（不是 404）——空数组是正常态。
#[get("/open/announces")]
pub(super) async fn open_announces(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<OpenQuery>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    let limit = q.limit.unwrap_or(100).clamp(1, MAX_ROWS);
    let since_id = q.since_id.unwrap_or(0);
    let since_ts = q.since.unwrap_or(0);
    let cats = CatMap::load(&state.repo.db).await;
    type Row = (
        i64,
        String,
        Option<String>,
        String,
        Option<String>,
        i64,
        i64,
        i32,
        i32,
        i32,
    );
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, info_hash, pieces_hash, name, small_descr, size, \
         EXTRACT(EPOCH FROM created_at)::bigint, seeders, leechers, \
         category_id \
         FROM torrents WHERE approval_status = 1 AND id > $1 \
           AND EXTRACT(EPOCH FROM created_at)::bigint >= $2 \
         ORDER BY id ASC LIMIT $3",
    )
    .bind(since_id)
    .bind(since_ts)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let next_since_id = rows.last().map(|r| r.0).unwrap_or(since_id);
    let count = rows.len();
    let dims = dims_for(
        &state.repo.db,
        &rows.iter().map(|r| r.0).collect::<Vec<_>>(),
    )
    .await;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, ih, ph, name, descr, size, ts, sd, lc, cat)| {
            let d = dims.get(&id).cloned().unwrap_or(
                serde_json::Map::new().into(),
            );
            project(id, ih, ph, name, descr, size, ts, sd, lc, cat, &cats, d)
        })
        .collect();
    Ok(with_rl(
        ok(serde_json::json!({
            "items": items,
            "next_since_id": next_since_id,
            "count": count,
        })),
        &tk,
    ))
}

/// GET /open/categories —— 分类与媒介字典（0268）。
///
/// 工具做分类下拉/筛选 UI 不再需要自备映射表：每条分类同时给出
/// 站内 id、NexusPHP 4xx 号与 Newznab 标准号（与列表/详情/Torznab 同源）。
#[get("/open/categories")]
pub(super) async fn open_categories(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_READ)?;
    let cats: Vec<(i32, String, i32, i32)> = sqlx::query_as(
        "SELECT id, name, COALESCE(legacy_id, id), \
         COALESCE(newznab_id, 8000) FROM categories ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let media: Vec<(i32, String)> =
        sqlx::query_as("SELECT id, name FROM media ORDER BY sort, id")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(with_rl(
        ok(serde_json::json!({
            "categories": cats.into_iter().map(|(id, name, np, nz)| {
                serde_json::json!({
                    "id": id, "name": name, "legacy_id": np, "newznab_id": nz,
                })
            }).collect::<Vec<_>>(),
            "media": media.into_iter().map(|(id, name)| {
                serde_json::json!({ "id": id, "name": name })
            }).collect::<Vec<_>>(),
        })),
        &tk,
    ))
}

#[get("/openapi.json")]
pub(super) async fn openapi_spec() -> impl Responder {
    ok(super::spec::build())
}
