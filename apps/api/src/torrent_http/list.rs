//! 种子列表（M02）：/torrents 主查询接口。
//! 从 torrent_http.rs 按域拆出；查询结构 ListQuery 与宽松反序列化助手在 query.rs。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::query::{norm_date, norm_promo, norm_text, parse_size, ListQuery};

/// 列表共享缓存的**代际号**键。写路径（发布/编辑/批量/审核/删除）INCR 它，
/// 缓存 key 带上代际 → 旧条目立即不再命中，靠自身 TTL 自然过期。
/// 不用 SCAN 删键，也修掉了「站长改完分类，列表仍显旧值 45 秒」。
const TLIST_GEN_KEY: &str = "cache:tlist:gen";

/// 当前代际（Redis 故障按 0 处理：退化成直查旧键，不影响可用性）
pub async fn list_cache_gen(state: &AppState) -> i64 {
    let mut c = state.redis.clone();
    let gen: Option<i64> = redis::AsyncCommands::get(&mut c, TLIST_GEN_KEY)
        .await
        .unwrap_or(None);
    gen.unwrap_or(0)
}

/// 列表内容发生变化时调用
pub async fn bump_list_cache_gen(state: &AppState) {
    let mut c = state.redis.clone();
    let _: Result<i64, _> =
        redis::AsyncCommands::incr(&mut c, TLIST_GEN_KEY, 1i64).await;
}

#[get("/torrents")]
async fn list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ListQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?; // 站点准入收口：资源元数据不对外
                                                  // 标签多选（0159 P1）：norm_tags 在 filter 构造块里赋值；声明在此因
                                                  // struct 字面量内不能先解构再引用同名字段
    let (tag_ids, tag_all) =
        super::query::norm_tags(q.tag_id, &q.tag_ids, q.tag_mode.as_deref());
    // 多维筛选（B3 六类型）：`sec_{kind}` / `_min` / `_max`，解析见 sec_params.rs
    let sections = super::sec_params::parse_section_params(
        &state.repo.read_db,
        req.query_string(),
    )
    .await;
    // 0170 站点开关：默认视图（approval 未指定 / 0）是否放行「审核中/失败」种子。
    // 审核状态视图（1=通过 2=被拒）不受开关影响；单条查询两列点查，主键命中极便宜。
    let default_view = q.approval.is_none() || q.approval == Some(0);
    let (show_pending, show_rejected) = if default_view {
        sqlx::query_as::<_, (bool, bool)>(
            "SELECT COALESCE((SELECT value = 'yes' FROM site_settings \
             WHERE name = 'list_show_pending'), false), \
             COALESCE((SELECT value = 'yes' FROM site_settings \
             WHERE name = 'list_show_rejected'), false)",
        )
        .fetch_one(&state.repo.read_db)
        .await
        .unwrap_or((false, false))
    } else {
        (false, false)
    };
    // 「能看未过审种子」这一条权限在多处生效（include_unapproved / approval /
    // 首屏缓存判定），先算一次，避免各分支各判各的而漏掉一个入口。
    let can_see_unapproved = crate::authz::can(
        &state,
        &auth,
        crate::authz::perm::TORRENT_SEE_BANNED,
    )
    .await;
    let filter = torrents::TorrentFilter {
        // 多选分类：重复参数或逗号串（category_id=1&category_id=3 / category_id=1,3）均解析。
        // 旧键名 category_id 单值也在此合并（category_id_alias 旁路收集）。
        category_id: {
            let mut raw: Vec<&str> =
                q.category_ids.iter().map(|s| s.as_str()).collect();
            if let Some(one) = q.category_id_alias.as_deref() {
                raw.push(one);
            }
            // 空串是表单未选的常态（category_id=），按「不筛选」处理；但给了值却
            // 解析不出来的必须报错——以前 filter_map 会静默丢掉，全非法时得到
            // None，等于「不加分类谓词 = 返回全站」，比 400 危险得多。
            let toks: Vec<&str> = raw
                .iter()
                .flat_map(|s| s.split(','))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            let mut ids: Vec<i32> = Vec::with_capacity(toks.len());
            for t in toks {
                let v = t.parse::<i32>().map_err(|_| {
                    DomainError::Validation(format!("分类参数非法：{t}"))
                })?;
                if !ids.contains(&v) {
                    ids.push(v);
                }
            }
            // 分类层级（0188 R4.3）：命中的分类递归展开子孙——筛父级包含
            // 子级种子；无层级时子孙为空、行为与平表完全一致
            let expanded: Vec<i32> = if ids.is_empty() {
                Vec::new()
            } else {
                let rows: Vec<i32> = sqlx::query_scalar(
                    "WITH RECURSIVE sub AS (                      SELECT id FROM categories WHERE id = ANY($1)                      UNION                      SELECT c.id FROM categories c                      JOIN sub s ON c.parent_id = s.id                      ) SELECT id FROM sub",
                )
                .bind(&ids)
                .fetch_all(&state.repo.read_db)
                .await
                .unwrap_or_default();
                let mut all = ids.clone();
                for r in rows {
                    if !all.contains(&r) {
                        all.push(r);
                    }
                }
                all
            };
            (!expanded.is_empty()).then_some(expanded)
        },
        medium_id: q.medium_id,
        grade_id: q.grade_id,
        edition_id: q.edition_id,
        official: q.official,
        include_dead: q.include_dead.unwrap_or(false),
        // 仅持 see_banned 权限者可查看未过审种子；无权限时参数被静默忽略
        include_unapproved: q.include_unapproved.unwrap_or(false)
            && can_see_unapproved,
        search: q.search.as_deref().map(str::to_string),
        sort: q.sort.as_deref().map(str::to_string),
        // 标签多选（0159 P1）：旧单值 tag_id 并入 tag_ids；tag_mode 只认 any/all
        tag_ids,
        tag_all,
        // 0102 高级搜索三态
        alive: q.alive,
        status: q.status.clone().filter(|s| !s.is_empty()),
        approval: q.approval.filter(|_| {
            // `?approval=2` 会把被拒与软删（回收站）种子直接给出去，
            // 与 include_unapproved 同一闸门：非 see_banned 者一律忽略该参数。
            can_see_unapproved
        }),

        // 0170 站点开关（仅默认视图生效，见上方读取）
        show_pending,
        show_rejected,
        // 0170 双向翻页：dir=prev 且带游标才生效（第一页没有「上一页」）
        reverse: q.dir.as_deref() == Some("prev")
            && q.cursor.as_deref().is_some_and(|c| !c.is_empty()),
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
        bookmarked: q.bookmarked.unwrap_or(false),
        sections,
        // 0267：前台列表没有「标题必须命中」入口，由对外端点（Torznab）使用
        title_like: None,
        // 评分下界（0283 P1-5）：越界（≤0 或 >10）丢弃 = 不筛
        rating_min: q
            .rating_min
            .filter(|v| *v > 0.0 && *v <= 10.0)
            .map(|v| (v * 10.0).round() / 10.0),
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
    // 游标（方案批次二）：「排序键值 + id」的 keyset，与 ORDER BY 同序——
    // 修复此前只比 id 导致的「非默认排序翻页丢行」。旧格式（纯数字）兼容解析。
    let cursor = match q.cursor.as_deref() {
        Some(c) if !c.is_empty() => Some(
            torrents::ListCursor::parse(c)
                .ok_or(DomainError::Validation("cursor 无效".into()))?,
        ),
        _ => None,
    };
    // P1-4 读缓存（0071）：仅覆盖「首屏等价视图」——无筛选/无搜索/无游标/默认排序的第 1 页。
    // 该视图不含用户视角字段（owner 匿名化在 SQL 层完成），全部登录用户看到的字节一致，可共享缓存。
    // TTL 45s 兜底 + Redis 故障直查；带任何筛选条件时不走缓存（避免失效风暴复杂化）。
    // 0170：反向页（reverse 需游标，逻辑上已被 cursor.is_none 排除，显式再挡一道）与
    // 开关放行的视图不走首屏缓存 —— cache_key 固定，开关开了再写会让关闭后 45s 内 stale。
    let is_first_screen = crate::torrents::view_scope::shared_cache_safe(
        &filter,
        cursor.is_none(),
        q.limit,
    );
    let gen = list_cache_gen(&state).await;
    let cache_key = format!("cache:tlist:first:v1:{gen}");
    if is_first_screen {
        let mut c = state.redis.clone();
        let hit: Option<String> = redis::AsyncCommands::get(&mut c, &cache_key)
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
            &state.repo.read_db,
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
    // 扩展共享缓存（方案 P0-3）：此前只有「纯净首屏」走缓存，分类浏览/搜索/排序/翻页
    // 等主路径全部穿透。凡不含用户视角语义（status/mine/bookmarked 三种 viewer 维度，
    // 判定见 view_scope::viewer_scoped）的
    // 请求，其结果对所有登录用户字节一致（owner 匿名化在 SQL 层完成、include_unapproved
    // 已按权限归一进 filter），可按「filter+cursor+limit」哈希共享。TTL 20s 兜底，
    // Redis 故障/序列化失败一律直查。
    // 0170：show_pending/show_rejected/reverse 均在 filter 内，序列化自动进 hash key。
    if !crate::torrents::view_scope::viewer_scoped(&filter) {
        if let Ok(canonical) =
            serde_json::to_string(&(&filter, &cursor, q.limit.unwrap_or(20)))
        {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            std::hash::Hash::hash(&canonical, &mut hasher);
            let shared_key = format!(
                "cache:tlist:v2:{gen}:{:016x}",
                std::hash::Hasher::finish(&hasher)
            );
            let mut c = state.redis.clone();
            let hit: Option<String> =
                redis::AsyncCommands::get(&mut c, &shared_key)
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
                &state.repo.read_db,
                &filter,
                cursor,
                q.limit.unwrap_or(20),
                auth.id,
            )
            .await?;
            if let Ok(json) = serde_json::to_string(&page) {
                let _: Result<(), _> = redis::AsyncCommands::set_ex(
                    &mut c, shared_key, json, 20u64,
                )
                .await;
            }
            return Ok(ok(page));
        }
    }
    let page = torrents::list_torrents_as(
        &state.repo.read_db,
        &filter,
        cursor,
        q.limit.unwrap_or(20),
        auth.id,
    )
    .await?;
    Ok(ok(page))
}
