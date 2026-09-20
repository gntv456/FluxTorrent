//! 种子浏览与互动（M02/M03/M07/M10）：列表/详情/文件/评论/感谢/收藏/
//! 编辑/定价/恢复/重提/删除/抓取列表/NFO/求续种/标签。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」第六段）。

use actix_web::{
    delete, get, post, put, web, HttpRequest, HttpResponse, Responder,
};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

// ============ 种子（M02/M03/M07/M10） ============

/// 宽松布尔解析（0093）：查询串里的 `1/0/true/false/yes/no` 均接受。
/// 此前 `include_dead=1`（旧站 1/0 口径、第三方客户端常用）会让整个 Query 反序列化失败 → 400。
fn de_bool_lenient<'de, D>(d: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let v = Option::<String>::deserialize(d)?;
    Ok(v.map(|s| {
        matches!(
            s.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    }))
}

/// 宽容的数字反序列化（005 修复）：表单里「全部」这类选项会提交 `alive=&tag_id=` 空串，
/// 而 `Option<i32>` 直接吃空串会解析失败 → 整个 Query 反序列化报错 → 400。
/// 这里统一把空/空白视为 None，非法值才算错。
fn de_opt_num_lenient<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let v = Option::<String>::deserialize(d)?;
    match v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => s.parse::<T>().map(Some).map_err(serde::de::Error::custom),
    }
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    medium_id: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    grade_id: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    edition_id: Option<i32>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    official: Option<bool>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    include_dead: Option<bool>,
    #[serde(default, deserialize_with = "de_bool_lenient")]
    include_unapproved: Option<bool>,
    search: Option<String>,
    /// 搜索范围：0=标题(默认) 1=副标题/简介 3=发布者 4=IMDb
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    search_area: Option<i32>,
    /// 匹配模式：0=AND 模糊(默认) 2=精确
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    search_mode: Option<i32>,
    sort: Option<String>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    tag_id: Option<i32>,
    /// 存活筛选（0102，NP inclbooked/vivisect 口径）：0=全部 1=仅活种 2=仅断种（覆盖 include_dead）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    alive: Option<i16>,
    /// 种子状态（0102，NP 口径）：seeding=当前做种 leeching=当前下载 completed=完成
    /// incomplete=未完成 notseeding=未做种（多值逗号串待扩展，先单值）
    #[serde(default)]
    status: Option<String>,
    /// 审核状态（0102）：0=全部 1=通过 2=被拒（含未审需 see_banned，另走 include_unapproved）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    approval: Option<i16>,
    /// 分类多选（0088）：`category_ids`（原生键，重复参数成 seq）与 `category_id`
    /// （前端/NP 旧键名，单值或逗号串）统一收集——serde rename_all/alias 对
    /// query-string 的重复键行为不可靠，这里显式两个键各收一份再合并去重。
    #[serde(default, deserialize_with = "de_category_ids_lenient")]
    category_ids: Vec<String>,
    /// category_id 旧键名的旁路收集（与上合并；空则不影响）
    #[serde(default, skip_serializing, rename = "category_id")]
    category_id_alias: Option<String>,
    cursor: Option<String>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    limit: Option<i64>,
    // ===== 高级搜索增强 =====
    /// 体积区间：支持 `1024`（字节）`500MB` `1.5GB` `2TB` 等带单位写法（见 parse_size）
    #[serde(default)]
    size_min: Option<String>,
    #[serde(default)]
    size_max: Option<String>,
    /// 发布时间区间（`YYYY-MM-DD`，按日历日口径，含边界当天）
    #[serde(default)]
    date_from: Option<String>,
    #[serde(default)]
    date_to: Option<String>,
    /// 做种数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    min_seeders: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    max_seeders: Option<i32>,
    /// 排除关键字（标题/简介均不得命中）
    #[serde(default)]
    exclude: Option<String>,
    /// 优惠筛选：free / x2 / half / any / none（白名单校验）
    #[serde(default)]
    promo: Option<String>,
    /// 发布者用户名（模糊匹配）
    #[serde(default)]
    owner: Option<String>,
    /// 仅显示我发布的种子
    #[serde(default, deserialize_with = "de_bool_lenient")]
    mine: Option<bool>,
    // ===== 高级搜索补齐（0118）：下载数/完成数区间 + 匿名发布 =====
    /// 下载数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    min_leechers: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    max_leechers: Option<i32>,
    /// 完成数区间（含边界）
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    min_completed: Option<i32>,
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    max_completed: Option<i32>,
    /// 匿名发布：0=全部(默认) 1=仅匿名 2=仅具名
    #[serde(default, deserialize_with = "de_opt_num_lenient")]
    anonymous: Option<i16>,
}

/// 日期参数校验（严格 `YYYY-MM-DD`）：非法值直接丢弃——宁可不筛，
/// 也不让 PG 对烂字符串抛错变成 500。
fn norm_date(s: Option<String>) -> Option<String> {
    let s = s?;
    let s = s.trim();
    let ok = s.len() == 10
        && s.as_bytes().iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    ok.then(|| s.to_string())
}

/// 优惠参数白名单（未知取值丢弃，避免静默返回空列表让用户以为「没数据」）
fn norm_promo(s: Option<String>) -> Option<String> {
    let s = s?.trim().to_ascii_lowercase();
    matches!(s.as_str(), "free" | "x2" | "half" | "any" | "none").then_some(s)
}

/// 空白即视为未填（表单里清空后仍会提交空串）
fn norm_text(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// 体积参数解析：`1024`（字节）/ `500MB` / `1.5GB` / `2TB`，KB/MB/GB/TB 与 KiB/GiB 写法均可，
/// 大小写不敏感。无效值或非正数返回 None（= 不筛，而不是报错）。
fn parse_size(s: Option<String>) -> Option<i64> {
    let raw = s?.trim().to_ascii_lowercase();
    if raw.is_empty() {
        return None;
    }
    let (num, mul) = if let Some(v) =
        raw.strip_suffix("tb").or_else(|| raw.strip_suffix("tib"))
    {
        (v, 1024f64.powi(4))
    } else if let Some(v) =
        raw.strip_suffix("gb").or_else(|| raw.strip_suffix("gib"))
    {
        (v, 1024f64.powi(3))
    } else if let Some(v) =
        raw.strip_suffix("mb").or_else(|| raw.strip_suffix("mib"))
    {
        (v, 1024f64.powi(2))
    } else if let Some(v) =
        raw.strip_suffix("kb").or_else(|| raw.strip_suffix("kib"))
    {
        (v, 1024f64)
    } else if let Some(v) = raw.strip_suffix('b') {
        (v, 1f64)
    } else {
        (raw.as_str(), 1f64)
    };
    let n: f64 = num.trim().parse().ok()?;
    let bytes = (n * mul).round();
    (bytes >= 1.0 && n.is_finite() && bytes < i64::MAX as f64)
        .then_some(bytes as i64)
}

/// category_ids/category_id 的宽松反序列化：seq → 原样；字符串 → 按逗号拆。
/// （此前用 alias 兼容单值键名，但 serde 对 Vec 字段的裸字符串直接报错 → 前端筛选 400）
fn de_category_ids_lenient<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        Many(Vec<String>),
        One(String),
    }
    // actix-web Query 的 serde_qs 形状：字段值是「单值或 seq」的直接载荷
    let v = serde_json::Value::deserialize(d)?;
    let pick = serde_json::from_value::<OneOrMany>(v)
        .map_err(serde::de::Error::custom)?;
    Ok(match pick {
        OneOrMany::Many(v) => v,
        OneOrMany::One(s) => {
            s.split(',').map(|x| x.trim().to_string()).collect()
        }
    })
}

#[get("/torrents")]
async fn list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?; // 站点准入收口：资源元数据不对外
                                                  // 通用多维筛选（0088）：sec_{kind}=dict_id，kind 走 section_kinds 白名单；
                                                  // 从原始 query string 解析，支持任意站方自建维度（不再写死六个）
    let mut sections: Vec<(String, i64)> = Vec::new();
    for pair in req.query_string().split('&') {
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        let Some(kind) = k.strip_prefix("sec_") else {
            continue;
        };
        if kind.is_empty() || v.is_empty() {
            continue;
        }
        if !crate::admin_p3_http::is_custom_kind(&state.repo.db, kind).await {
            continue;
        }
        // 多选（0102）：逗号串 / 重复参数均可；同维度多值 = OR（任一命中）
        for part in v.split(',') {
            if let Ok(dict_id) = part.trim().parse::<i64>() {
                sections.push((kind.to_string(), dict_id));
            }
        }
    }
    let filter = torrents::TorrentFilter {
        // 多选分类：重复参数或逗号串（category_id=1&category_id=3 / category_id=1,3）均解析。
        // 旧键名 category_id 单值也在此合并（category_id_alias 旁路收集）。
        category_id: {
            let mut raw: Vec<&str> =
                q.category_ids.iter().map(|s| s.as_str()).collect();
            if let Some(one) = q.category_id_alias.as_deref() {
                raw.push(one);
            }
            let ids: Vec<i32> = raw
                .iter()
                .flat_map(|s| s.split(','))
                .filter_map(|s| s.trim().parse::<i32>().ok())
                .collect();
            (!ids.is_empty()).then_some(ids)
        },
        medium_id: q.medium_id,
        grade_id: q.grade_id,
        edition_id: q.edition_id,
        official: q.official,
        include_dead: q.include_dead.unwrap_or(false),
        // 仅持 see_banned 权限者可查看未过审种子；无权限时参数被静默忽略
        include_unapproved: q.include_unapproved.unwrap_or(false)
            && crate::authz::can(
                &state,
                &auth,
                crate::authz::perm::TORRENT_SEE_BANNED,
            )
            .await,
        search: q.search.as_deref().map(str::to_string),
        sort: q.sort.as_deref().map(str::to_string),
        tag_id: q.tag_id,
        // 0102 高级搜索三态
        alive: q.alive,
        status: q.status.clone().filter(|s| !s.is_empty()),
        approval: q.approval,
        // 搜索盒口径（此前前端传了但后端不解析，静默失效）
        search_area: q.search_area,
        search_mode: q.search_mode,
        // 高级搜索增强（体积/时间/做种数/排除词/优惠/发布者/仅我发布）
        size_min: parse_size(q.size_min.clone()),
        size_max: parse_size(q.size_max.clone()),
        date_from: norm_date(q.date_from.clone()),
        date_to: norm_date(q.date_to.clone()),
        min_seeders: q.min_seeders.filter(|v| *v >= 0),
        max_seeders: q.max_seeders.filter(|v| *v >= 0),
        exclude: norm_text(q.exclude.clone()),
        promo: norm_promo(q.promo.clone()),
        owner: norm_text(q.owner.clone()),
        only_mine: q.mine.unwrap_or(false),
        // 0118 高级搜索补齐（下载数/完成数区间 + 匿名发布；负数丢弃 = 不筛）
        min_leechers: q.min_leechers.filter(|v| *v >= 0),
        max_leechers: q.max_leechers.filter(|v| *v >= 0),
        min_completed: q.min_completed.filter(|v| *v >= 0),
        max_completed: q.max_completed.filter(|v| *v >= 0),
        anonymous: q.anonymous.filter(|v| (0..=2).contains(v)),
        sections,
    };
    // 上下界颠倒时自动对调（用户先填大后填小很常见，直接判空更友好）
    let mut filter = filter;
    if let (Some(a), Some(b)) = (filter.size_min, filter.size_max) {
        if a > b {
            filter.size_min = Some(b);
            filter.size_max = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (filter.min_seeders, filter.max_seeders) {
        if a > b {
            filter.min_seeders = Some(b);
            filter.max_seeders = Some(a);
        }
    }
    if let (Some(a), Some(b)) =
        (filter.date_from.clone(), filter.date_to.clone())
    {
        if a > b {
            filter.date_from = Some(b);
            filter.date_to = Some(a);
        }
    }
    // 0118：下载数/完成数区间同样自动对调
    if let (Some(a), Some(b)) = (filter.min_leechers, filter.max_leechers) {
        if a > b {
            filter.min_leechers = Some(b);
            filter.max_leechers = Some(a);
        }
    }
    if let (Some(a), Some(b)) = (filter.min_completed, filter.max_completed) {
        if a > b {
            filter.min_completed = Some(b);
            filter.max_completed = Some(a);
        }
    }
    let cursor = match q.cursor.as_deref() {
        Some(c) if !c.is_empty() => Some(
            c.parse::<i64>()
                .map_err(|_| DomainError::Validation("cursor 无效".into()))?,
        ),
        _ => None,
    };
    // P1-4 读缓存（0071）：仅覆盖「首屏等价视图」——无筛选/无搜索/无游标/默认排序的第 1 页。
    // 该视图不含用户视角字段（owner 匿名化在 SQL 层完成），全部登录用户看到的字节一致，可共享缓存。
    // TTL 45s 兜底 + Redis 故障直查；带任何筛选条件时不走缓存（避免失效风暴复杂化）。
    let is_first_screen = cursor.is_none()
        && filter.category_id.is_none()
        && filter.medium_id.is_none()
        && filter.grade_id.is_none()
        && filter.edition_id.is_none()
        && filter.official.is_none()
        && !filter.include_dead
        && !filter.include_unapproved
        && filter.search.is_none()
        && filter.sort.is_none()
        && filter.tag_id.is_none()
        && filter.sections.is_empty()
        // 状态筛选是用户视角（viewer 的 snatches）：带 status 的请求不得共享首屏缓存
        && filter.status.is_none()
        && filter.alive.is_none()
        // 0105 高级搜索增强：任一条件生效即视为非「首屏等价视图」，不得复用共享缓存
        && filter.size_min.is_none()
        && filter.size_max.is_none()
        && filter.date_from.is_none()
        && filter.date_to.is_none()
        && filter.min_seeders.is_none()
        && filter.max_seeders.is_none()
        && filter.exclude.is_none()
        && filter.promo.is_none()
        && filter.owner.is_none()
        && !filter.only_mine
        // 0118 补齐项同口径：任一生效即非首屏等价视图
        && filter.min_leechers.is_none()
        && filter.max_leechers.is_none()
        && filter.min_completed.is_none()
        && filter.max_completed.is_none()
        && filter.anonymous.is_none()
        && q.limit.unwrap_or(20) == 20;
    let cache_key = "cache:tlist:first:v1";
    if is_first_screen {
        let mut c = state.redis.clone();
        let hit: Option<String> = redis::AsyncCommands::get(&mut c, cache_key)
            .await
            .unwrap_or(None);
        if let Some(json) = hit {
            if let Ok(page) =
                serde_json::from_str::<torrents::TorrentPage>(&json)
            {
                return Ok(ok(page));
            }
        }
        let page = torrents::list_torrents_as(
            &state.repo.db,
            &filter,
            cursor,
            q.limit.unwrap_or(20),
            auth.id,
        )
        .await?;
        if let Ok(json) = serde_json::to_string(&page) {
            let _: Result<(), _> =
                redis::AsyncCommands::set_ex(&mut c, cache_key, json, 45u64)
                    .await;
        }
        return Ok(ok(page));
    }
    let page = torrents::list_torrents_as(
        &state.repo.db,
        &filter,
        cursor,
        q.limit.unwrap_or(20),
        auth.id,
    )
    .await?;
    Ok(ok(page))
}

#[get("/torrents/{id}")]
async fn detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 持 view_anonymous 权限者可见匿名种子的真实发布者
    let reveal = crate::authz::can(
        &state,
        &auth,
        crate::authz::perm::TORRENT_VIEW_ANONYMOUS,
    )
    .await;
    // G7：staff 视角传 (uid, true)——暂缓种对 staff 开放
    let viewer = (auth.id, auth.class_id >= 90);
    let t = torrents::get_torrent(
        &state.repo.db,
        path.into_inner(),
        reveal,
        Some(viewer),
    )
    .await?;
    Ok(ok(t))
}

/// 详情页扩展数据（简介/文件数/感谢数），与 detail 合并渲染
#[get("/torrents/{id}/detail")]
async fn torrent_detail_ext(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let t = torrents::get_torrent_detail(
        &state.repo.db,
        path.into_inner(),
        auth.id,
    )
    .await?;
    Ok(ok(t))
}

#[get("/torrents/{id}/files")]
async fn torrent_files(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(
        torrents::list_files(&state.repo.db, path.into_inner()).await?
    ))
}

#[get("/torrents/{id}/thanks")]
async fn torrent_thanks(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(
        torrents::list_thanks(&state.repo.db, path.into_inner()).await?
    ))
}

#[get("/torrents/{id}/comments")]
async fn comments(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    let items = torrents::list_comments(
        &state.repo.db,
        path.into_inner(),
        q.limit.unwrap_or(20),
    )
    .await?;
    Ok(ok(items))
}

#[derive(Deserialize)]
struct CommentReq {
    body: String,
}

#[post("/torrents/{id}/comments")]
async fn create_comment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<CommentReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = torrents::add_comment(
        &state.repo.db,
        path.into_inner(),
        auth.id,
        &body.body,
    )
    .await?;
    Ok(ok(serde_json::json!({ "id": id })))
}

/// 审计修复（P1）：评论此前只有创建+列表，作者本人与版主都无法删除（无任何删除端点）。
/// 作者本人或持 torrent.manage 的 staff 可删；挂审计日志。
#[delete("/torrents/{id}/comments/{cid}")]
async fn delete_comment(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<(i64, i64)>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (torrent_id, comment_id) = path.into_inner();
    let owner: Option<i64> = sqlx::query_scalar(
        "SELECT user_id FROM comments WHERE id = $1 AND torrent_id = $2",
    )
    .bind(comment_id)
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten();
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(comment_id));
    };
    let is_owner = owner_id == auth.id;
    let is_manager =
        crate::authz::can(&state, &auth, crate::authz::perm::TORRENT_MANAGE)
            .await;
    if !is_owner && !is_manager {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM comments WHERE id = $1")
        .bind(comment_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(
            Some(auth.id),
            if is_owner {
                "comment.delete_own"
            } else {
                "comment.delete_staff"
            },
            Some(comment_id),
        )
        .await;
    Ok(ok(serde_json::json!({ "deleted": comment_id })))
}

#[derive(Deserialize)]
struct ThankBody {
    /// 魔力答谢数额（馒头口径：+1/+10/+100/+500/+1000/+10000；缺省 0 = 免费感谢）
    #[serde(default)]
    amount: Option<i64>,
}

#[post("/torrents/{id}/thanks")]
async fn do_thank(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: Option<web::Json<ThankBody>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::thank(&state.repo.db, tid, auth.id).await?;
    let amount = body.and_then(|b| b.amount).unwrap_or(0);
    if amount > 0 {
        if ![1, 10, 100, 500, 1000, 10000].contains(&amount) {
            return Err(DomainError::Validation(
                "答谢数额需为 1/10/100/500/1000/10000".into(),
            ));
        }
        // 给发布者转魔力（匿名也按 owner_id 记账）
        let owner: Option<i64> =
            sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
                .bind(tid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten();
        if let Some(owner) = owner {
            if owner != auth.id {
                let idem = format!(
                    "thank-spark:{}:{}:{}",
                    auth.id,
                    tid,
                    chrono::Utc::now().timestamp()
                );
                crate::economy_http::earn_spark(
                    &state.repo.db,
                    owner,
                    amount,
                    "task_reward",
                    &idem,
                )
                .await?;
            }
        }
    }
    Ok(ok(
        serde_json::json!({ "thanked": true, "spark_given": amount }),
    ))
}

#[derive(Deserialize)]
struct BookmarkReq {
    on: bool,
}

/// 编辑种子（NP edit/takeedit 作者口径；修改后回退待审）
#[derive(Deserialize)]
struct TorrentEditReq {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    small_descr: Option<String>,
    #[serde(default)]
    descr: Option<String>,
    #[serde(default)]
    anonymous: Option<bool>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    medium_id: Option<i32>,
    #[serde(default)]
    grade_id: Option<i32>,
    #[serde(default)]
    edition_id: Option<i32>,
}

#[put("/torrents/{id}")]
async fn edit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TorrentEditReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::edit_torrent(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id as i16),
        &torrents::TorrentEdit {
            name: body
                .name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty()),
            small_descr: body.small_descr.as_deref(),
            descr: body.descr.as_deref(),
            anonymous: body.anonymous,
            category_id: body.category_id,
            medium_id: body.medium_id,
            grade_id: body.grade_id,
            edition_id: body.edition_id,
        },
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.edit", Some(tid))
        .await;
    Ok(ok(
        serde_json::json!({ "edited": true, "note": "已回退待审核" }),
    ))
}

/// 修改付费定价（0086）：发布者或 staff；改价不影响已购（以 torrent_purchases 已扣为准）
#[derive(serde::Deserialize)]
struct PriceReq {
    price: i64,
}

#[put("/torrents/{id}/price")]
async fn set_torrent_price(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<PriceReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let price = body.price.clamp(0, 1_000_000);
    sqlx::query("UPDATE torrents SET price = $2 WHERE id = $1")
        .bind(id)
        .bind(price)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "torrent.price_set", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "price": price })))
}

/// 恢复软删种子（approval_status 3 → 0 待审）：此前误删后只能直连数据库手工修数
#[post("/torrents/{id}/restore")]
async fn restore_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_MANAGE,
    )
    .await?;
    let id = path.into_inner();
    torrents::restore_torrent(&state.repo.db, id).await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.restore", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "restored": id })))
}

/// 被拒种子修改后重提（U4 §12.5 审核闭环：rejected → pending，保留原 id 与评论区）。
/// 仅作者本人；清拒绝标记（deny_reason/note），重进审核队列。
#[post("/torrents/{id}/resubmit")]
async fn resubmit_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    let owner: Option<(i64, i16)> = sqlx::query_as(
        "SELECT owner_id, approval_status FROM torrents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, status)) = owner else {
        return Err(DomainError::NotFound(id));
    };
    if owner_id != auth.id {
        return Err(DomainError::Forbidden);
    }
    if status != 2 {
        return Err(DomainError::Validation(
            "仅被拒种子可重提（当前状态不符）".into(),
        ));
    }
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, deny_reason_id = NULL, deny_note = NULL \
         WHERE id = $1 AND approval_status = 2",
    )
    .bind(id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("状态已变化，请刷新".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "torrent.resubmit", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "resubmitted": id })))
}

/// 删除种子（软删 approval_status=3；staff 任意删，作者仅限未过审）
#[delete("/torrents/{id}")]
async fn delete_torrent(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    torrents::delete_torrent(
        &state.repo.db,
        id,
        (auth.id, auth.class_id as i16),
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.delete", Some(id))
        .await;
    Ok(ok(serde_json::json!({ "deleted": id })))
}

/// 下载/做种记录（NP viewsnatches.php）
#[get("/torrents/{id}/snatches")]
async fn torrent_snatches(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(torrents::list_snatches(
        &state.repo.db,
        path.into_inner(),
    )
    .await?))
}

/// NFO（NP viewnfo.php）
#[get("/torrents/{id}/nfo")]
async fn torrent_nfo(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    require_auth(&req, &state).await?;
    let nfo = torrents::get_nfo(&state.repo.db, path.into_inner()).await?;
    Ok(ok(serde_json::json!({ "nfo": nfo })))
}

/// 请求补种（NP takereseed.php：死种 → PM 全体完成者，900s 限频）
#[post("/torrents/{id}/reseed")]
async fn request_reseed(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    let username: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or_else(|_| "user".into());
    let n = torrents::request_reseed(&state.repo.db, tid, (auth.id, username))
        .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.reseed", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "notified": n })))
}

/// 种子标签（T-04）：字典 + 已打
#[get("/torrents/{id}/tags")]
async fn torrent_tags(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    Ok(ok(
        torrents::list_tags(&state.repo.db, path.into_inner()).await?
    ))
}

#[derive(Deserialize)]
struct TagReq {
    tag_id: i32,
    on: bool,
}

#[put("/torrents/{id}/tags")]
async fn torrent_tag_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<TagReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let tid = path.into_inner();
    torrents::tag_torrent(
        &state.repo.db,
        tid,
        (auth.id, auth.class_id as i16),
        body.tag_id,
        body.on,
    )
    .await?;
    state
        .repo
        .audit(Some(auth.id), "torrent.tag", Some(tid))
        .await;
    Ok(ok(serde_json::json!({ "tag": body.tag_id, "on": body.on })))
}

#[put("/torrents/{id}/bookmark")]
async fn do_bookmark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<BookmarkReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    torrents::bookmark(&state.repo.db, path.into_inner(), auth.id, body.on)
        .await?;
    Ok(ok(serde_json::json!({ "bookmarked": body.on })))
}
