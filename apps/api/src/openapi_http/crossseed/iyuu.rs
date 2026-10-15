// ============ IYUU 协议适配 ============
// 协议来源（2026-10-15 核实，不是凭印象写的）：IYUUPlus 开源客户端
// `ledccn/iyuuplus-dev` 的 `composer/reseed-client/src/Client.php`
// 与 `composer/site-manager/src/Contracts/Response.php`。
//
// **先纠正此前的设计误解**：原注释把本站当成「向 IYUU 中央服务器
// 查询的客户端」。实际协议是反的——IYUUPlus 客户端自己抓各站详情页，
// 把结果 POST 给中央服务器，中央返回别站的下载链接；而**各 PT 站只
// 需要「长得像 IYUU 认识的站」**：有稳定的详情页 URL 模式、能被
// 爬虫解析出 info_hash / 下载链接。本站不是中央服务器，也不该去
// 代理别人的站点——那既没意义也越权。
//
// 所以这里的适配层只做三件**确实是本站该做的**事：
//
// 1. `POST /crossseed/iyuu` —— 接受 IYUU 格式的批量指纹查询
//    （`{hash: [...], sha1: [...]}`，字段名与 IYUU 客户端的
//    `/reseed/index/index` 一致），翻译成 lookup 内核的
//    `{info_hash_v1, pieces_sha1}`，并把结果**反向**折成 IYUU 的
//    `{payload, metadata}` 形状（`metadata=false` = 下载链接，
//    与 Response 契约一致）。这样 IYUU 的中央服务器或任何按该
//    形状消费的对接方都能直接用本站做数据源。
// 2. `GET /crossseed/iyuu/site` —— 站点自描述：详情页 URL 模式、
//    下载 URL 模式、passkey 字段名。这是 IYUU 爬虫识别本站所需的
//    全部信息（IYUU 的 `sites/index` 就是这个形状）。
// 3. 下载链接一律走 `/crossseed/dl/{id}` —— 复用既有 30 分钟临时
//    凭证链路（限流/资格/扣费全在 compat 侧），不造第二套口径。
//
// 不做的事：不代理其它 PT 站、不代抓别人页面、不做 sid_sha1 绑定
// （那是 IYUU 中央服务器的职责，本站不冒充）。

use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};

use super::{norm_hashes, daily_quota, with_rl, require_scope, require_token,
           MAX_FINGERPRINTS, RL_PER_MIN, SCOPE_CROSSSEED};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// IYUU 请求体（字段名与 IYUU 客户端 `/reseed/index/index` 对齐）。
#[derive(Deserialize, utoipa::ToSchema)]
pub struct IyuuReq {
    /// v1 info_hash 列表（hex40，大小写不敏感）。
    #[serde(default)]
    pub hash: Vec<String>,
    /// pieces 层 SHA1 列表（hex40）。
    #[serde(default)]
    pub sha1: Vec<String>,
    /// 客户端站点绑定哈希（IYUU 中央签发；本站不校验，仅回显以便排查）。
    #[serde(default)]
    pub sid_sha1: Option<String>,
}

/// IYUU 单条结果（形状取自 `Iyuu\SiteManager\Contracts\Response`：
/// `payload` + `metadata`；`metadata=true` 表示 payload 是二进制种子
/// 字节，`false` 表示它是下载链接）。
#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct IyuuHit {
    /// 下载链接（本站取 `/crossseed/dl/{id}`，走 30 分钟临时凭证）。
    pub payload: String,
    /// 恒 false：本站只发链接，不直传二进制种子（直传会让 30 分钟
    /// 凭证的限流/扣费口径绕过——那正是 dl 端点存在的理由）。
    pub metadata: bool,
    /// 附带字段（IYUU 客户端会忽略；给人看的排查信息）。
    pub torrent_id: i64,
    pub name: String,
    pub size: i64,
    pub info_hash: String,
}

/// POST /crossseed/iyuu —— IYUU 格式的辅种查询。
///
/// 与 `/crossseed/lookup` 共用同一内核与同一套闸门（scope、10 次/分、
/// 5 万指纹/日、hex40 校验、已审核可见性），只是**外壳**不同：入参
/// 字段名换成 IYUU 的 `hash`/`sha1`，出参折成 `payload`/`metadata`。
/// 两套外壳一份内核——避免出现「两个出口两套口径」那种漂移。
#[utoipa::path(
    post,
    path = "/api/v1/crossseed/iyuu",
    tag = "辅种(crossseed)",
    description = "IYUU 协议适配：入参 {hash[], sha1[]}（IYUU 客户端字段名），\
出参 {payload, metadata} 形状（IYUU Response 契约）。内核与 /crossseed/lookup \
完全一致（scope/限流/日配额/已审核可见性共用），只换外壳。",
    request_body = IyuuReq,
    responses(
        (status = 200, description = "data.hits：命中项（IYUU Response 形状）"),
        (status = 400, description = "指纹非 hex40 / 两组合计超 500 / 全空（code 1002）"),
        (status = 401, description = "Token 无效/未传（code 2001）"),
        (status = 403, description = "Token 无 crossseed scope（code 2003）"),
        (status = 429, description = "频率或日配额超限（code 1015）"),
    )
)]
#[actix_web::post("/crossseed/iyuu")]
pub async fn xseed_iyuu(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<IyuuReq>,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    require_scope(&tk, SCOPE_CROSSSEED)?;
    let v1 = norm_hashes(&body.hash)?;
    let ph = norm_hashes(&body.sha1)?;
    if v1.is_empty() && ph.is_empty() {
        return Err(DomainError::Validation(
            "hash 与 sha1 不能都为空".into(),
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
        &format!("xseed:iyuu:{}", tk.uid),
        RL_PER_MIN,
        60,
    )
    .await?;
    daily_quota(&state, tk.uid, (v1.len() + ph.len()) as i64).await?;
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
    // 折成 IYUU 形状：payload = 下载链接（metadata 恒 false）。
    // 与 lookup 一致：info_hash 命中优先归类，只影响附带信息不影响链接。
    let hits: Vec<IyuuHit> = rows
        .iter()
        .map(|(id, name, size, ih, _ph)| IyuuHit {
            payload: format!("/api/v1/crossseed/dl/{id}"),
            metadata: false,
            torrent_id: *id,
            name: name.clone(),
            size: *size,
            info_hash: ih.clone(),
        })
        .collect();
    Ok(with_rl(
        ok(serde_json::json!({
            "hits": hits,
            // 回显 sid_sha1 便于客户端对齐（本站不据此鉴权，见文件头注释）
            "sid_sha1": body.sid_sha1,
        })),
        &tk,
    ))
}

/// GET /crossseed/iyuu/site —— 站点自描述（IYUU `sites/index` 形状）。
///
/// IYUU 爬虫靠这份描述认出「这个站怎么抓」：详情页 URL 模板、下载
/// 参数名、是否需要 passkey。本站走 NexusPHP 兼容路径，模板用
/// `/torrents.php?id={id}` / `/torrents.php?id={id}&action=download`。
///
/// 不鉴权：这是公开的站点元数据（IYUU 的 `sites/index` 也不鉴权），
/// 且不含任何凭据——passkey 字段名是常量，不是某个用户的 key。
#[utoipa::path(
    get,
    path = "/api/v1/crossseed/iyuu/site",
    tag = "辅种(crossseed)",
    description = "站点自描述（IYUU sites/index 形状）：详情页/下载 URL 模板与 \
passkey 字段名。公开元数据，不含凭据。",
    responses((status = 200, description = "站点描述对象"))
)]
#[actix_web::get("/crossseed/iyuu/site")]
pub async fn xseed_iyuu_site(
    _req: HttpRequest,
) -> impl actix_web::Responder {
    // 站名/域名从环境读（与仓库既有 PUBLIC_SITE_URL 口径一致），
    // 读不到就给占位——**不给错域名**，让 IYUU 侧报「未识别」而不是
    // 拿着错的域名去爬。
    let base = std::env::var("PUBLIC_SITE_URL")
        .unwrap_or_else(|_| "{PUBLIC_SITE_URL}".into());
    actix_web::HttpResponse::Ok().json(serde_json::json!({
        "site": "fluxtorrent",
        "name": "FluxTorrent",
        "base_url": base,
        // NexusPHP 兼容路由：详情与下载同入口，下载走 action=download
        "detail_url": base.trim_end_matches('/').to_string() + "/torrents.php?id={torrent_id}",
        "download_url": base.trim_end_matches('/').to_string()
            + "/torrents.php?id={torrent_id}&action=download",
        // passkey 参数名（NexusPHP 生态通行口径）
        "passkey_field": "passkey",
        "auth": "passkey",
        // 本站辅种走 API 凭证（30 分钟临时 key），下载链接形态同上
        "download_via_api": true,
        "api_download_path": "/api/v1/crossseed/dl/{torrent_id}",
        "protocol_version": "2.0.0",
    }))
}