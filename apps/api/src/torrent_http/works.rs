//! 作品聚合读口（资源页「作品聚合」+ 作品详情）。
//!
//! 一个 **作品** = 同一部片/剧的多个版本（4K / 1080P / 不同压制组）。
//! 归属口径见 [`super::works_sql`]：优先 `torrents.group_id`，否则按归一化
//! 标题兜底——不新建实体、不需要站务额外维护。
//!
//! 端点：
//!   · `GET /works?...`  作品列表（分页；筛选参数与 `/torrents` 同源）
//!   · `GET /works/{key}` 作品详情 + 种子行 + 相关
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
use crate::torrents::promo;
use crate::torrents::{search_pred_with_syn, section_where};

use super::works_filter;
use super::works_rows_sql::{self, BatchSeedRow, WorkSeed};
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
    pub seeds: Vec<WorkSeed>,
}

/// 工品列表查询：`q` 是作品关键字（兼容列表 search），其余筛选直接复用
/// `/torrents` 的 ListQuery 字段（分类多选/标签/维度/数值/排序/视角…）。
#[derive(Deserialize)]
pub struct WorksQuery {
    #[serde(flatten)]
    pub filters: super::query::ListQuery,
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
}

/// 排序白名单：用户串只用来查表，永不内插（注入面为零）。
/// 支持列表同款 `_asc` 后缀；作品层没有 comments 聚合，落到默认。
fn order_of(sort: Option<&str>) -> String {
    let (key, asc) = match sort.map(str::trim) {
        Some(s) if s.ends_with("_asc") => (&s[..s.len() - 4], true),
        Some(s) => (s, false),
        None => ("", false),
    };
    let d = if asc { "ASC" } else { "DESC" };
    let tie = format!(", a.latest_at DESC, a.wk ASC");
    match key {
        "seeders" => format!("a.seeders {d}{tie}"),
        "leechers" => format!("a.leechers {d}{tie}"),
        "size" => format!("a.size {d}{tie}"),
        "torrents" => format!("a.torrents {d}{tie}"),
        "completed" => format!("a.completed {d}{tie}"),
        "name" => format!("r.name {d}{tie}"),
        "created" | "latest" | "" => {
            format!("a.latest_at {d}, a.wk ASC")
        }
        _ => "a.latest_at DESC, a.wk ASC".to_string(),
    }
}

/// 与 filter_sql 的 $1..$21 槽位逐位对应；列表再补 $22/$23。
macro_rules! bind_works_filter {
    ($query:expr, $f:expr, $viewer:expr) => {
        $query
            .bind($f.filter.category_id.clone())
            .bind($f.filter.official)
            .bind($f.filter.size_min)
            .bind($f.filter.size_max)
            .bind($f.filter.date_from.as_deref())
            .bind($f.pattern.as_deref())
            .bind($f.filter.date_to.as_deref())
            .bind($f.filter.min_seeders)
            .bind($f.filter.max_seeders)
            .bind($f.exclude_pat.as_deref())
            .bind($f.filter.promo.as_deref())
            .bind($f.owner_pat.as_deref())
            .bind($f.filter.only_mine.then_some($viewer))
            .bind($f.filter.min_leechers)
            .bind($f.filter.max_leechers)
            .bind($f.filter.min_completed)
            .bind($f.filter.max_completed)
            .bind($f.filter.anonymous)
            .bind($f.filter.rating_min)
            .bind($f.filter.tag_ids.clone())
            .bind(None::<String>)
    };
}

#[utoipa::path(
    get,
    path = "/api/v1/works",
    tag = "种子",
    description = "作品聚合列表（同片多版本归为一部作品；筛选与 /torrents 同源）。",
    params(
        ("q" = Option<String>, description = "关键字（模糊）"),
        ("category_id" = Option<String>, description = "分类 id，逗号多选"),
        ("sort" = Option<String>, description = "sort key; _asc reverses"),
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
    let auth = require_auth(&req, &state).await?;
    let limit = q.filters.limit.unwrap_or(24).clamp(1, 100);
    let page = q.page.unwrap_or(1).max(1);
    // 统一走分页助手：旧式手写偏移（页码减一乘每页条数）在极端页码下会
    // 绕成**负 offset**（release 构建）⇒ PG 报「OFFSET must not be negative」
    // 打成 500；助手把页码夹在 1..=1_000_000 并用饱和乘法 ⇒ 极端页码只是空页。
    let offset = crate::dto::page_offset(page, limit);
    let wf = works_filter::parse(
        &state.repo,
        req.query_string(),
        &q.filters,
        q.q.as_deref(),
    )
    .await?;
    let sec_sql = section_where(&state.repo.read_db, &wf.filter.sections).await;
    let search_pred = search_pred_with_syn(
        &state.repo.read_db,
        wf.filter.search.as_deref(),
        wf.filter.search_area,
        wf.filter.search_mode == Some(2),
        " ESCAPE chr(92)",
    )
    .await;
    let sort = order_of(q.filters.sort.as_deref());
    let sql =
        works_sql::list_sql(&sort, &wf.filter, &sec_sql, &search_pred, auth.id);
    let items: Vec<WorkRow> =
        bind_works_filter!(sqlx::query_as::<_, WorkRow>(&sql), &wf, auth.id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.repo.read_db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    let count_sql =
        works_sql::count_sql(&wf.filter, &sec_sql, &search_pred, auth.id);
    let total: i64 =
        bind_works_filter!(sqlx::query_scalar(&count_sql), &wf, auth.id)
            .fetch_one(&state.repo.read_db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    // 每部作品挂前 8 个种子（单条窗口函数查询，避免 N+1）
    let keys: Vec<String> = items.iter().map(|w| w.key.clone()).collect();
    let mut by: std::collections::HashMap<String, Vec<WorkSeed>> =
        std::collections::HashMap::new();
    if !keys.is_empty() {
        let seeds: Vec<BatchSeedRow> = sqlx::query_as(
            &works_rows_sql::batch_sql(&promo::lateral_latest()),
        )
        .bind(&keys)
        .fetch_all(&state.repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        for row in seeds {
            let (key, seed) = row.into_parts();
            by.entry(key).or_default().push(seed);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// /works 复用列表 ListQuery：扁平反序列化必须真的收下分类/标签/
    /// 存活/排序/评分这些参数，否则页面转发了也会静默失效。
    #[test]
    fn works_query_flattens_list_filters() {
        let q = web::Query::<WorksQuery>::from_query(
            "q=Audit&limit=5&page=2&category_id=1,2&alive=2\
             &tag_ids=3&tag_mode=all&sort=seeders_asc&rating_min=8",
        )
        .expect("query")
        .into_inner();
        assert_eq!(q.q.as_deref(), Some("Audit"));
        assert_eq!(q.page, Some(2));
        assert_eq!(q.filters.limit, Some(5));
        assert_eq!(q.filters.category_id_alias.as_deref(), Some("1,2"));
        assert_eq!(q.filters.alive, Some(2));
        assert_eq!(q.filters.tag_ids, vec!["3".to_string()]);
        assert_eq!(q.filters.tag_mode.as_deref(), Some("all"));
        assert_eq!(q.filters.sort.as_deref(), Some("seeders_asc"));
        assert_eq!(q.filters.rating_min, Some(8.0));
    }
}
