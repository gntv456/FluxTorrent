//! 种子列表（M02）：/torrents 主查询接口。
//! 从 torrent_http.rs 按域拆出；查询结构 ListQuery 与宽松反序列化助手在 query.rs。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

use super::query::{
    anonymous_or_err, date_or_err, norm_text, promo_or_err, size_or_err,
    ListQuery,
};

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
) -> DomainResult<impl Responder> {
    // 站点准入收口：资源元数据不对外
    let auth = require_auth(&req, &state).await?;
    // 参数抽取失败要回站内统一信封：此前 `web::Query<ListQuery>` 形参在抽取
    // 失败时由 actix 直接回 text/plain 400（无 code/message，前端只显未知错误；
    // 重复键 `?alive=0&alive=2`、`?limit=abc` 全中，2026-10-10 批九实测）。
    let q = web::Query::<ListQuery>::from_query(req.query_string())
        .map_err(|_| DomainError::Validation("列表筛选参数格式无效".into()))?
        .into_inner();
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
    // 审核状态视图（1=通过 2=被拒 3=回收站）不受开关影响；单条查询两列点查，主键命中极便宜。
    // `?approval=` 只认 0..=3：此前越界值（如 9）静默落到默认视图，
    // 伸手要回收站的人拿到一批过审种——答非所问还不报错。
    if let Some(v) = q.approval {
        if !(0..=3).contains(&v) {
            return Err(DomainError::Validation(
                "approval 取值无效：0 待审 / 1 过审 / 2 被拒 / 3 回收站".into(),
            ));
        }
    }
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
    // P-275：控制面板的 incl_dead / torrents_per_page 折进显式字段。优先级
    // ?alive= > ?include_dead= > 个人偏好——偏好是「我的默认视图」，
    // 不是「强制视图」，显式参数（含旧站工具口径）必须能压过它。
    let (incl_dead, per_page) =
        crate::repo::user_prefs::viewer_prefs(&state.repo, auth.id).await;
    let alive_pref = q
        .include_dead
        .map(|d| if d { 0 } else { 1 })
        .or_else(|| super::list_prefs::alive_from_pref(incl_dead));
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
        // 0102 高级搜索三态（URL 没给时用「显示断种/活种」偏好兜底）
        alive: q.alive.or(alive_pref),
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
        // P-279：给了值却解析不出来 ⇒ 400（旧口径静默不筛 = 返回全站）
        size_min: size_or_err(q.size_min.clone())?,
        size_max: size_or_err(q.size_max.clone())?,
        date_from: date_or_err(q.date_from.clone())?,
        date_to: date_or_err(q.date_to.clone())?,
        min_seeders: q.min_seeders.filter(|v| *v >= 0),
        max_seeders: q.max_seeders.filter(|v| *v >= 0),
        exclude: norm_text(q.exclude.clone()),
        // P-277：给了值却不认识 ⇒ 400（旧行为归一成 None = 不加谓词 = 全站）
        promo: promo_or_err(q.promo.clone())?,
        owner: norm_text(q.owner.clone()),
        only_mine: q.mine.unwrap_or(false),
        // 0118 高级搜索补齐（下载数/完成数区间 + 匿名发布；负数丢弃 = 不筛）
        min_leechers: q.min_leechers.filter(|v| *v >= 0),
        max_leechers: q.max_leechers.filter(|v| *v >= 0),
        min_completed: q.min_completed.filter(|v| *v >= 0),
        max_completed: q.max_completed.filter(|v| *v >= 0),
        anonymous: anonymous_or_err(q.anonymous)?,
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
    // 上下界颠倒时自动对调（口径与逐条判据见 list_prefs::swap_bounds）
    let mut filter = filter;
    super::list_prefs::swap_bounds(&mut filter);
    // 游标（方案批次二）：「排序键值 + id」的 keyset，与 ORDER BY 同序——
    // 修复此前只比 id 导致的「非默认排序翻页丢行」。旧格式（纯数字）兼容解析。
    let cursor = match q.cursor.as_deref() {
        Some(c) if !c.is_empty() => Some(
            torrents::ListCursor::parse(c)
                .ok_or(DomainError::Validation("cursor 无效".into()))?,
        ),
        _ => None,
    };
    // P-275：每页条数——URL 优先，其次「每页 X 条」偏好，都没有才 20
    let pref_limit = super::list_prefs::limit_from_pref(per_page);
    let limit = q.limit.or(pref_limit).unwrap_or(20);
    // P1-4 读缓存（0071）：仅覆盖「首屏等价视图」——无筛选/无搜索/无游标/默认排序的第 1 页。
    // 该视图不含用户视角字段（owner 匿名化在 SQL 层完成），全部登录用户看到的字节一致，可共享缓存。
    // TTL 45s 兜底 + Redis 故障直查；带任何筛选条件时不走缓存（避免失效风暴复杂化）。
    // 0170：反向页（reverse 需游标，逻辑上已被 cursor.is_none 排除，显式再挡一道）与
    // 开关放行的视图不走首屏缓存 —— cache_key 固定，开关开了再写会让关闭后 45s 内 stale。
    let is_first_screen = crate::torrents::view_scope::shared_cache_safe(
        &filter,
        cursor.is_none(),
        Some(limit),
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
            limit,
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
            serde_json::to_string(&(&filter, &cursor, limit))
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
                limit,
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
        limit,
        auth.id,
    )
    .await?;
    Ok(ok(page))
}
