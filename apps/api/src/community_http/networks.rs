//! 内容实体列表：出品方/厂牌聚合页（0330 documentary 专项 P1）。
//!
//! 承载决策见迁移 0330：`content_networks` 是**按名聚合的锚点表**，
//! 值仍存 `torrent_sections`（dict_id → section_dict.name），维度链路
//! 完全不动。本模块只提供读口：
//!   · `GET /networks`        厂牌列表（按名下过审种数排序，可搜索）
//!   · `GET /networks/{id}`   厂牌页（该厂牌全部过审种）
//!   · `GET /networks/top`    厂牌榜（按名下总做种数）
//!
//! 与 `artists` 的关键差别：network 是 **select 枚举**（值在
//! section_dict，sections 行走 dict_id），artist 是 text 自由值
//! （sections 行走 value 文本）。所以本模块的关联必须经
//! `section_dict` 反查，不能直接比 `ts.value`。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 厂牌名下过审种数的相关子查询（列表/榜单共用，单一真相源）。
/// 枚举维度：`torrent_sections.dict_id` → `section_dict.name` 匹配。
/// `$1` = kind 键（network/studio/…）。
const COUNT_SQL: &str = "SELECT count(DISTINCT ts.torrent_id) \
     FROM torrent_sections ts \
     JOIN section_dict sd ON sd.id = ts.dict_id \
     JOIN torrents t ON t.id = ts.torrent_id \
     WHERE ts.kind = $1 AND sd.name = cn.name \
       AND t.approval_status = 1";

#[derive(Deserialize)]
struct NetworkQuery {
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    limit: Option<i64>,
}

/// 厂牌列表（登录可读；搜索 + 分页；按名下过审种数降序）。
#[get("/networks")]
pub async fn networks_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<NetworkQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let offset = (q.page.unwrap_or(1).max(1) - 1) * limit;
    let kw = q.q.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let kind = q.kind.as_deref().unwrap_or("network");
    let sql = format!(
        "SELECT cn.id, cn.name, ({COUNT_SQL}) AS torrents \
         FROM content_networks cn \
         WHERE cn.kind = $1 \
           AND ($2::text IS NULL OR cn.name ILIKE '%' || $2 || '%') \
         ORDER BY 3 DESC, cn.norm_name LIMIT $3 OFFSET $4"
    );
    let rows: Vec<(i64, String, i64)> = sqlx::query_as(&sql)
        .bind(kind)
        .bind(kw)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM content_networks \
         WHERE kind = $1 \
           AND ($2::text IS NULL OR name ILIKE '%' || $2 || '%')",
    )
    .bind(kind)
    .bind(&kw)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(rows.len() as i64);
    Ok(ok(serde_json::json!({
        "items": rows.iter().map(|(id, name, n)| serde_json::json!({
            "id": id, "name": name, "torrents": n,
        })).collect::<Vec<_>>(),
        "total": total,
        "kind": kind,
    })))
}

/// 厂牌页：该厂牌名下全部过审种（时间倒序）。
#[get("/networks/{id}")]
pub async fn network_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT kind, name FROM content_networks WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((kind, name)) = row else {
        return Err(DomainError::NotFound(id));
    };
    let items: Vec<(i64, String, i64, i32, i32)> = sqlx::query_as(
        "SELECT DISTINCT t.id, t.name, t.size, t.seeders, \
                t.times_completed \
         FROM torrents t \
         JOIN torrent_sections ts ON ts.torrent_id = t.id \
         JOIN section_dict sd ON sd.id = ts.dict_id \
         WHERE ts.kind = $1 AND sd.name = $2 \
           AND t.approval_status = 1 \
         ORDER BY t.id DESC LIMIT 100",
    )
    .bind(&kind)
    .bind(&name)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "id": id, "kind": kind, "name": name,
        "items": items.iter().map(|(tid, tn, sz, sd, tc)| {
            serde_json::json!({
                "id": tid, "name": tn, "size": sz,
                "seeders": sd, "times_completed": tc,
            })
        }).collect::<Vec<_>>(),
    })))
}

/// 厂牌榜（按名下总做种数降序）——Gazelle top labels 范式的最小实现。
#[get("/networks/top")]
pub async fn networks_top(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<NetworkQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let kind = q.kind.as_deref().unwrap_or("network");
    let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
        "SELECT cn.id, cn.name, \
         (SELECT count(DISTINCT ts.torrent_id) FROM torrent_sections ts \
            JOIN section_dict sd ON sd.id = ts.dict_id \
            JOIN torrents t ON t.id = ts.torrent_id \
           WHERE ts.kind = $1 AND sd.name = cn.name \
             AND t.approval_status = 1) AS releases, \
         (SELECT COALESCE(SUM(t.seeders), 0) FROM torrents t \
            JOIN torrent_sections ts ON ts.torrent_id = t.id \
            JOIN section_dict sd ON sd.id = ts.dict_id \
           WHERE ts.kind = $1 AND sd.name = cn.name \
             AND t.approval_status = 1) AS seeders \
         FROM content_networks cn \
         WHERE cn.kind = $1 \
         ORDER BY seeders DESC, releases DESC, cn.norm_name LIMIT 20",
    )
    .bind(kind)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows
        .iter()
        .map(|(id, name, rel, sd)| {
            serde_json::json!({
                "id": id, "name": name,
                "releases": rel, "seeders": sd,
            })
        })
        .collect::<Vec<_>>()))
}
