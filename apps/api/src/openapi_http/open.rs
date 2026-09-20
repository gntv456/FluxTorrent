//! 开放数据接口（Token 鉴权）与 OpenAPI 文档（与实现同步维护）。
//! 从 openapi_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::require_token;

/// 开放接口示例：最新种子（RSS 增强客户端/移动壳可用的稳定只读契约）
#[get("/open/recent")]
pub(super) async fn open_recent_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let (_uid, _rpm) = require_token(&req, &state).await?;
    // U2 §11.6 访客策略：all_private（默认）下公开列表收敛为空——
    // Token 鉴权「接口调用者」，guest_policy 管「站点公开度」，两层独立。
    let policy: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'guest_policy'), 'all_private')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "all_private".into());
    if policy == "all_private" {
        return Ok(ok(Vec::<serde_json::Value>::new()));
    }
    let rows: Vec<(i64, String, Option<String>, i64, i64)> = sqlx::query_as(
        "SELECT id, name, small_descr, size, created_epoch \
         FROM (SELECT id, name, small_descr, size, EXTRACT(EPOCH FROM created_at)::bigint AS created_epoch \
               FROM torrents WHERE approval_status = 1 ORDER BY id DESC LIMIT 50) t",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, name, descr, size, ts)| {
            serde_json::json!({
                "id": id, "name": name, "description": descr,
                "size": size, "published_at": ts,
            })
        })
        .collect();
    Ok(ok(items))
}

#[get("/openapi.json")]
pub(super) async fn openapi_spec() -> impl Responder {
    ok(serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "FluxTorrent Open API",
            "version": "1.0.0",
            "description": "第三方只读接口（M27）。鉴权：Authorization: Token <fxo_...>。Token 在网页端「我的 → API Token」签发。"
        },
        "servers": [{ "url": "/api/v1" }],
        "components": {
            "securitySchemes": {
                "apiToken": { "type": "apiKey", "in": "header", "name": "Authorization", "description": "值形如 `Token fxo_xxx`" }
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
                        "size": { "type": "integer", "format": "int64" },
                        "published_at": { "type": "integer", "format": "int64", "description": "Unix 秒" }
                    }
                }
            }
        },
        "security": [{ "apiToken": [] }],
        "paths": {
            "/open/recent": {
                "get": {
                    "summary": "最新 50 枚种子",
                    "description": "按 id 倒序的种子摘要，适配 RSS 增强客户端与移动壳。",
                    "responses": {
                        "200": {
                            "description": "Envelope<TorrentSummary[]>",
                            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/Envelope" } } }
                        },
                        "401": { "description": "token 无效或已撤销（code 2001）" },
                        "429": { "description": "超出 token 独立限流（code 1015）" }
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
