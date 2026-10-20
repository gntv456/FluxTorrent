//! 作品聚合读口（资源页「作品聚合」+ 作品详情）。
//!
//! 一个 **作品** = 同一部片/剧的多个版本（4K / 1080P / 不同压制组）。
//! 归属口径见 [`super::works_sql`]：优先 `torrents.group_id`，否则按归一化
//! 标题兜底——不新建实体、不需要站务额外维护。
//!
//! 端点：
//!   · `GET /works?q=&category_id=&sort=&limit=&page=`  作品列表（分页）
//!   · `GET /works/{key}`                               作品详情 + 种子行 + 相关
//!
//! 两处都要登录（与 `/torrents` 同口径：资源元数据不对外）。
//! 作品键 `g12` / `n<hash>` 由 SQL 单源产出，前端原样回拼即可。

use actix_web::{get, web, HttpRequest, Responder};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{require_auth, Q};
use crate::state::AppState;
use crate::torrents::{esc_like, promo, TorrentRow};

use super::works_rows_sql;
use super::works_sql;

/// 作品行（列表与详情头同形）。字段刻意贴 TorrentListItem 口径
/// （poster/rating/tags 直接复用种子行的 JSON 形状），前端一套渲染通吃。
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct WorkRow {
    /// 作品键：`g{group_id}` 或 `n{md5(归一化标题)}`
    pub key: String,
    /// 作品名（组内最新一条的片名，展示时会再洗一遍）
    pub name: String,
    pub poster: Option<String>,
    pub rating: Option<String>,
    pub category_id: i32,
    /// 简介（组内最新一条的小简介；没有就空）
    pub synopsis: Option<String>,
    /// 完整简介（组内最新一条的 descr；纯文本渲染，前端不注水 HTML）
    pub detail: Option<String>,
    /// 外部条目 ID（发种时归一化；见 publish_http/upload_ids.rs）。
    /// 作品键就是它们生成的：`i<tt…>` / `m<tmdb>` / `d<douban>`
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<String>,
    /// TMDB 条目链接（movie/tv 路径只有链接里才有；缺省按 movie 拼）
    pub tmdb_url: Option<String>,
    pub douban_id: Option<String>,
    /// 作品条目缓存（work_meta 0399）：导演/主演/类型/时长/地区/评分的
    /// 正式来源；`source` 为 null 表示还没回查过，前端走 media_info 回落
    pub meta: Option<serde_json::Value>,
    /// 年份（从片名里抓的第一处 19xx/20xx）
    pub year: Option<i32>,
    /// 作品内种子数
    pub torrents: i64,
    pub seeders: i64,
    pub leechers: i64,
    pub size: i64,
    pub latest_at: chrono::DateTime<chrono::Utc>,
    /// 组内最新种子 id（「下载/查看」的兜底落点）
    pub top_id: i64,
    /// 标签徽标（与列表行同形：id/name/kind/bg_color/color）
    pub tags: Value,
}

/// 列表项 = 作品行 + 该作品的前若干种子（样图里每张作品卡下面直接挂种子行）。
#[derive(Debug, Serialize)]
pub struct WorkWithSeeds {
    #[serde(flatten)]
    pub work: WorkRow,
    pub seeds: Vec<TorrentRow>,
}

/// 批量行查询的中间行：作品键 + 种子行（sqlx 0.8 的 flatten 展开列集）
#[derive(Debug, sqlx::FromRow)]
struct SeedRow {
    work_key: String,
    #[sqlx(flatten)]
    tor: TorrentRow,
}

#[derive(Debug, Deserialize)]
pub struct WorksQuery {
    /// 关键字（匹配组内任一种子名，模糊）
    #[serde(default)]
    pub q: Option<String>,
    /// 兼容 `/torrents?search=` 的写法
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub category_id: Option<i32>,
    /// latest（默认）/ seeders / size / torrents
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub page: Option<i64>,
}

/// 排序白名单：用户串只用来查表，永不内插（注入面为零）。
fn order_of(sort: Option<&str>) -> &'static str {
    match sort.map(str::trim) {
        Some("seeders") => "a.seeders DESC, a.latest_at DESC",
        Some("size") => "a.size DESC, a.latest_at DESC",
        Some("torrents") => "a.torrents DESC, a.latest_at DESC",
        _ => "a.latest_at DESC, a.wk ASC",
    }
}

/// 关键字 → ILIKE 模式（`%`/`_` 转义，与列表搜索同口径）
fn like_of(q: &WorksQuery) -> Option<String> {
    let raw = q.q.as_deref().or(q.search.as_deref())?;
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    Some(format!("%{}%", esc_like(s)))
}

#[utoipa::path(
    get,
    path = "/api/v1/works",
    tag = "种子",
    description = "作品聚合列表（同片多版本归为一部作品；group_id 优先，\
            否则按归一化标题兜底）。",
    params(
        ("q" = Option<String>, description = "关键字（模糊）"),
        ("category_id" = Option<i32>, description = "分类"),
        ("sort" = Option<String>, description = "latest/seeders/size/torrents"),
        ("limit" = Option<i64>, description = "每页（默认 24，上限 100）"),
        ("page" = Option<i64>, description = "页码，从 1 起"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[get("/works")]
pub(crate) async fn works_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: Q<WorksQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(24).clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    let offset = (page - 1) * limit;
    let pat = like_of(&q);

    let items: Vec<WorkRow> = sqlx::query_as(&works_sql::list_sql(order_of(
        q.sort.as_deref(),
    )))
    .bind(q.category_id)
    .bind(&pat)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let total: i64 = sqlx::query_scalar(&works_sql::count_sql())
        .bind(q.category_id)
        .bind(&pat)
        .fetch_one(&state.repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 每部作品挂前 8 个种子（单条窗口函数查询，避免 N+1）
    let keys: Vec<String> = items.iter().map(|w| w.key.clone()).collect();
    let mut by: std::collections::HashMap<String, Vec<TorrentRow>> =
        std::collections::HashMap::new();
    if !keys.is_empty() {
        let seeds: Vec<SeedRow> =
            sqlx::query_as(&works_rows_sql::batch_sql(&promo::lateral_latest()))
                .bind(&keys)
                .fetch_all(&state.repo.read_db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        for s in seeds {
            by.entry(s.work_key).or_default().push(s.tor);
        }
    }
    let out: Vec<WorkWithSeeds> = items
        .into_iter()
        .map(|work| {
            let seeds = by.remove(&work.key).unwrap_or_default();
            WorkWithSeeds { work, seeds }
        })
        .collect();

    Ok(ok(serde_json::json!({
        "items": out,
        "total": total,
        "limit": limit,
        "page": page,
    })))
}

/// 作品详情：头部（作品行）+ 该作品的种子行 + 同分类相关作品。
#[utoipa::path(
    get,
    path = "/api/v1/works/{key}",
    tag = "种子",
    description = "作品详情：作品头 + 作品内种子行（与列表行同形）+ 相关作品。",
    responses(
        (status = 200, description = "ok"),
        (status = 404, description = "作品不存在或无可见种子（code 1004）"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
#[get("/works/{key}")]
pub(crate) async fn work_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let key = path.into_inner();
    // 键形态白名单：`g` + 数字 或 `n` + 32 位十六进制。拼错/乱填直接 404，
    // 不放它去跑一次全表聚合。
    let digit =
        |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    let valid = match key.as_bytes().first() {
        Some(b'g') => key[1..].parse::<i64>().is_ok(),
        // IMDb 键：`i` + tt…（大小写不敏感的比较交给 SQL，这里只认形态）
        Some(b'i') => key.len() > 3 && key[1..].starts_with(['t', 'T']),
        Some(b'm') | Some(b'd') => digit(&key[1..]),
        Some(b'n') => {
            key.len() == 33 && key[1..].bytes().all(|c| c.is_ascii_hexdigit())
        }
        _ => false,
    };
    if !valid {
        return Err(DomainError::NotFound(0));
    }

    let work: Option<WorkRow> = sqlx::query_as(&works_sql::one_sql())
        .bind(&key)
        .fetch_optional(&state.repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(work) = work else {
        return Err(DomainError::NotFound(0));
    };

    let items: Vec<TorrentRow> =
        sqlx::query_as(&works_rows_sql::rows_sql(&promo::lateral_latest()))
            .bind(&key)
            .fetch_all(&state.repo.read_db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    let related: Vec<WorkRow> = sqlx::query_as(&works_sql::related_sql())
        .bind(work.category_id)
        .bind(&key)
        .fetch_all(&state.repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "work": work,
        "items": items,
        "related": related,
    })))
}
