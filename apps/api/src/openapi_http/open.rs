//! 开放数据接口（Token 鉴权）与 OpenAPI 文档（与实现同步维护）。
//! 从 openapi_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;
use crate::torrents::CatMap;

use super::{require_token, with_rl};

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
    })
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
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, ih, ph, name, descr, size, ts, sd, lc, cat)| {
            project(id, ih, ph, name, descr, size, ts, sd, lc, cat, &cats)
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
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, ih, ph, name, descr, size, ts, sd, lc, cat)| {
            project(id, ih, ph, name, descr, size, ts, sd, lc, cat, &cats)
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

#[get("/openapi.json")]
pub(super) async fn openapi_spec() -> impl Responder {
    ok(serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "FluxTorrent Open API",
            "version": "1.1.0",
            "description": "第三方接口。鉴权：Authorization: Token <fxo_...>。\
    Token 在网页端「我的 → API Token」签发，180 天，可 POST /me/tokens/refresh 续期。\
    响应带 X-RateLimit-Limit/Remaining/Reset 头；超限返回 429（code 1015）+ Retry-After。"
        },
        "servers": [{ "url": "/api/v1" }],
        "components": {
            "securitySchemes": {
                "apiToken": { "type": "apiKey", "in": "header", "name": "Authorization", "description": "值形如 `Token fxo_xxx`；亦支持 `?apikey=`" }
            },
            "schemas": {
                "Envelope": {
                    "type": "object",
                    "properties": {
                        "code": { "type": "integer" },
                        "message": { "type": "string" },
                        "data": {},
                        "request_id": { "type": "string", "format": "uuid" }
                    }
                },
                "TorrentSummary": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer", "format": "int64" },
                        "name": { "type": "string" },
                        "description": { "type": "string", "nullable": true },
                        "size": { "type": "integer", "format": "int64", "description": "字节" },
                        "published_at": { "type": "integer", "format": "int64", "description": "Unix 秒" },
                        "seeders": { "type": "integer" },
                        "leechers": { "type": "integer" },
                        "category": { "type": "integer" },
                        "category_name": { "type": "string" },
                        "category_np": { "type": "integer", "description": "NexusPHP 4xx 口径分类号" },
                        "category_newznab": { "type": "integer", "description": "Newznab/Torznab 标准分类号" },
                        "info_hash": { "type": "string", "description": "40 位 hex" },
                        "pieces_hash": { "type": "string", "nullable": true, "description": "跨站辅种指纹" }
                    }
                }
            }
        },
        "security": [{ "apiToken": [] }],
        "paths": {
            "/open/recent": {
                "get": {
                    "summary": "最新种子",
                    "description": "按 id 倒序的种子摘要；limit 1-200（默认 50）。",
                    "parameters": [
                        { "name": "limit", "in": "query", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": { "description": "Envelope<TorrentSummary[]>" },
                        "401": { "description": "token 无效或已撤销（code 2001）" },
                        "429": { "description": "超出 token 独立限流（code 1015）" }
                    }
                }
            },
            "/open/announces": {
                "get": {
                    "summary": "增量新种流",
                    "description": "返回 id > since_id 的过审种子（升序），用于只推新种的轮询；把响应的 next_since_id 存下来当下次游标。",
                    "parameters": [
                        { "name": "since_id", "in": "query", "schema": { "type": "integer" } },
                        { "name": "since", "in": "query", "schema": { "type": "integer", "description": "Unix 秒下界" } },
                        { "name": "limit", "in": "query", "schema": { "type": "integer" } }
                    ],
                    "responses": {
                        "200": { "description": "Envelope<{items, next_since_id, count}>" }
                    }
                }
            },
            "/open/torrents": {
                "post": {
                    "summary": "Token 化发种",
                    "description": "multipart/form-data：file=<.torrent>（必填）、nfo=<文本>；其余元数据走查询参数（category_id、name、small_descr、descr、anonymous、price…）。按 info_hash/pieces_hash 幂等：已存在时返回 200 且 duplicate=true。",
                    "requestBody": {
                        "content": { "multipart/form-data": { "schema": { "type": "object" } } }
                    },
                    "responses": {
                        "200": { "description": "Envelope<{id, approval_status, auto_approved, duplicate, info_hash, pieces_hash}>" },
                        "401": { "description": "token 无效（code 2001）" },
                        "403": { "description": "无发种权限（code 2003）" },
                        "400": { "description": "种子无效/元数据非法（code 3003/1002）" }
                    }
                }
            },
            "/openapi.json": {
                "get": {
                    "summary": "本文档",
                    "security": [],
                    "responses": { "200": { "description": "OpenAPI 3.1 文档" } }
                }
            }
        }
    }))
}
