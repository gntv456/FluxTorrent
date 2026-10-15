//! M36 crossseed-api：辅种（cross-seed）指纹批量查询 + 下载。
//! 对标 PT Studio FR-XSEED-001~009（_doc/PT-Studio终局能力与FluxTorrent借鉴评估.md）。
//!
//! 数据侧**零迁移**：`info_hash CHAR(40) UNIQUE`（0001）+ `pieces_hash CHAR(40)`
//! （0069 建表注释即「跨站辅种二级指纹」，同批建了部分索引）——本模块只是
//! 在这两列上包一层鉴权 API。scope 独立（`crossseed`），与发种（upload）解耦。
//!
//! 端点：
//! - `POST /crossseed/lookup`：双指纹批量查询（≤500/次，10 次/分钟，
//!   5 万指纹/日），只返回已审核种（可见性口径照抄详情：approval_status=1）；
//! - `GET /crossseed/dl/{id}`：现场签发 30 分钟临时凭证并 302 到现有
//!   下载端点（限流/资格/扣费全走 compat 既有链路，不造第二套）；
//! - `POST /crossseed/iyuu`：IYUU 协议适配**未定型**（请求/响应格式需对照
//!   其公开文档核实，PT Studio 同样列为部署前核实项）——返回 501，
//!   好过返回错格式。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use std::collections::BTreeSet;

use super::{
    no_store, require_scope, require_token, with_rl, SCOPE_CROSSSEED,
};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 单次请求指纹总数上限（两数组合计；PT Studio 同值）。
const MAX_FINGERPRINTS: usize = 500;
/// 频率：10 次/分钟（PT Studio 同值）。
const RL_PER_MIN: i64 = 10;
/// 日配额：50,000 指纹/日（Redis 计数器，兼任每 token 查询量审计）。
const DAILY_QUOTA: i64 = 50_000;
/// 日配额键 TTL：48h（跨日余量，天然回收）。
const DAILY_TTL_SECS: i64 = 48 * 3600;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct XseedLookupReq {
    /// v1 btih（hex40；大小写不敏感，服务端归一）。与 pieces_sha1 合计 ≤500。
    #[serde(default)]
    pub info_hash_v1: Vec<String>,
    /// pieces 层 SHA1（hex40）：跨站重打包后 info_hash 变、此值不变，
    /// 辅种的核心匹配维度。
    #[serde(default)]
    pub pieces_sha1: Vec<String>,
}

/// 归一化 + 校验 + 去重：hex40 小写集合。非 hex40 一律 400
/// （fail-closed：畸形指纹不放进查询，也不要在错误里回显原文）。
fn norm_hashes(raw: &[String]) -> DomainResult<BTreeSet<String>> {
    let mut out = BTreeSet::new();
    for h in raw {
        let h = h.trim().to_ascii_lowercase();
        if h.len() != 40 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(DomainError::Validation(
                "指纹须为 40 位十六进制（hex40）".into(),
            ));
        }
        out.insert(h);
    }
    Ok(out)
}

/// 日配额：Redis INCR（fail-close——Redis 抖动即拒，不当没配额）。
/// 首次写入设 TTL；返回值为本次写入后的累计量。
async fn daily_quota(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
    n: i64,
) -> DomainResult<()> {
    use redis::AsyncCommands;
    let day = chrono::Utc::now().format("%Y%m%d");
    let k = format!("xseed:day:{uid}:{day}");
    let mut c = state.redis.clone();
    let used: i64 =
        c.incr(&k, n).await.map_err(|e| DomainError::Internal(e.into()))?;
    // 键此前不存在时 INCR 返回值即 n —— 此刻补 TTL（48h 跨日余量）
    if used <= n {
        let _: redis::RedisResult<()> = c.expire(&k, DAILY_TTL_SECS).await;
    }
    if used > DAILY_QUOTA {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

/// POST /crossseed/lookup —— 双指纹批量查询。
/// 响应只含命中项（客户端按 query 集合差集判未命中）；
/// 每条结果回标 `match`（哪个入参数命中）与 `query`（原指纹）。
#[utoipa::path(
    post,
    path = "/api/v1/crossseed/lookup",
    tag = "辅种(crossseed)",
    description = "辅种指纹批量查询：info_hash_v1 + pieces_sha1 两类合计 ≤500 条，\
10 次/分钟、5 万指纹/日。只返回已审核种（approval_status=1，与详情同口径）；\
未命中的 query 不出现在 results。下载走响应里的 download_url。",
    request_body = XseedLookupReq,
    responses(
        (status = 200, description = "results：命中项数组"),
        (status = 400, description = "指纹非 hex40 / 两组合计超 500 / 全空（code 1002）"),
        (status = 401, description = "Token 无效/未传（code 2001）"),
        (status = 403, description = "Token 无 crossseed scope（code 2003）"),
        (status = 429, description = "频率或日配额超限（code 1015）"),
    )
)]
#[post("/crossseed/lookup")]
async fn xseed_lookup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<XseedLookupReq>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_CROSSSEED)?;
    let v1 = norm_hashes(&body.info_hash_v1)?;
    let ph = norm_hashes(&body.pieces_sha1)?;
    if v1.is_empty() && ph.is_empty() {
        return Err(DomainError::Validation(
            "info_hash_v1 与 pieces_sha1 不能都为空".into(),
        ));
    }
    if v1.len() + ph.len() > MAX_FINGERPRINTS {
        return Err(DomainError::Validation(format!(
            "指纹总数超限：{} > {MAX_FINGERPRINTS}（去重后合计）",
            v1.len() + ph.len()
        )));
    }
    crate::http::rate_window(
        &state,
        &format!("xseed:lk:{}", tk.uid),
        RL_PER_MIN,
        60,
    )
    .await?;
    daily_quota(&state, tk.uid, (v1.len() + ph.len()) as i64).await?;
    // 单条 SQL：两列各一次 ANY（都有索引），可见性口径 = 详情（已审核）
    let v1v: Vec<String> = v1.iter().cloned().collect();
    let phv: Vec<String> = ph.iter().cloned().collect();
    let rows: Vec<(i64, String, i64, String, Option<String>)> =
        sqlx::query_as(
            "SELECT id, name, size, info_hash, pieces_hash FROM torrents \
             WHERE approval_status = 1 \
               AND (info_hash = ANY($1::char(40)[]) \
                    OR pieces_hash = ANY($2::char(40)[]))",
        )
        .bind(&v1v)
        .bind(&phv)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 回标 match/query：v1 优先（主指纹），其次 pieces
    let results: Vec<_> = rows
        .iter()
        .map(|(id, name, size, ih, ph_)| {
            let (m, q) = if v1.contains(ih) {
                ("info_hash_v1", ih.clone())
            } else {
                ("pieces_sha1", ph_.clone().unwrap_or_else(|| ih.clone()))
            };
            serde_json::json!({
                "match": m,
                "query": q,
                "torrent_id": id,
                "name": name,
                "size": size,
                "download_url": format!("/api/v1/crossseed/dl/{id}"),
            })
        })
        .collect();
    Ok(with_rl(
        ok(serde_json::json!({ "results": results })),
        &tk,
    ))
}

/// GET /crossseed/dl/{torrent_id} —— 现场签发 30 分钟临时凭证，
/// 302 到既有凭证下载端点（限流/资格闸门/扣费/build_torrent_bytes
/// 全部复用 compat 链路，辅种下载不造第二套口径）。
#[utoipa::path(
    get,
    path = "/api/v1/crossseed/dl/{torrent_id}",
    tag = "辅种(crossseed)",
    description = "辅种下载：以 crossseed Token 现场签发 30 分钟临时凭证并 302 \
到 /api/v1/downloads/{id}（资格/扣费/构建走既有链路）。每用户 10 次/分钟。",
    params(("torrent_id" = i64, Path,
        description = "种子 id（lookup 结果里的 torrent_id）")),
    responses(
        (status = 302, description = "Location: /api/v1/downloads/{id}\
?token=<fxk_ 30 分钟凭证>"),
        (status = 401, description = "Token 无效/未传（code 2001）"),
        (status = 403, description = "Token 无 crossseed scope（code 2003）"),
        (status = 404, description = "种子不存在或未过审（code 1004）"),
        (status = 429, description = "限流（code 1015）"),
    )
)]
#[get("/crossseed/dl/{torrent_id}")]
async fn xseed_dl(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_CROSSSEED)?;
    crate::http::rate_window(
        &state,
        &format!("xseed:dl:{}", tk.uid),
        RL_PER_MIN,
        60,
    )
    .await?;
    let torrent_id = path.into_inner();
    let (plain, _expires) =
        crate::compat_http::download_keys::issue_download_key(
            &state,
            tk.uid,
            torrent_id,
        )
        .await?;
    // 一次性凭据在 Location query 里：禁中间层缓存
    Ok(no_store(
        HttpResponse::Found()
            .insert_header((
                actix_web::http::header::LOCATION,
                format!("/api/v1/downloads/{torrent_id}?token={plain}"),
            ))
            .finish(),
    ))
}

/// POST /crossseed/iyuu —— IYUU 协议适配（未定型，501）。
/// 设计稿纪律：请求/响应格式未对照 IYUU 公开文档核实前不写转换层，
/// 返回 501 好过返回错格式；定型后只包一层「协议适配 → lookup 内核」。
#[utoipa::path(
    post,
    path = "/api/v1/crossseed/iyuu",
    tag = "辅种(crossseed)",
    description = "IYUU 协议适配（未定型）：暂返回 501。字段格式需对照 IYUU \
公开对接文档 + IYUUPlus 客户端实际请求核实后再启用（PT Studio 同列为部署前核实项）。",
    responses(
        (status = 501, description = "协议适配未定型"),
        (status = 401, description = "Token 无效/未传（code 2001）"),
        (status = 403, description = "Token 无 crossseed scope（code 2003）"),
    )
)]
#[post("/crossseed/iyuu")]
async fn xseed_iyuu(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_CROSSSEED)?;
    Ok(with_rl(
        HttpResponse::NotImplemented()
            .content_type("application/json")
            .body(
                serde_json::json!({
                    "code": 0,
                    "message": "IYUU 协议适配未定型：字段格式待对照其公开\
对接文档核实后启用（lookup 内核已就绪，适配层不会重复实现业务）",
                    "data": null,
                })
                .to_string(),
            ),
        &tk,
    ))
}


#[cfg(test)]
mod tests;
