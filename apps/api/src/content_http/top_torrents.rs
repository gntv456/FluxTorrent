//! 内容热榜三件套（0424 P1-1）：今日热门 / 本周最多下载 / 新晋免费。
//!
//! 全站此前只有用户六榜（/top/boards），没有任何内容侧榜单——多数 PT 站
//! 「免费区 + 热门榜」是拉下载的第一入口。三榜口径：
//!   * today  = 今日（站点日 UTC+8）times_completed 增量最多的种
//!   * week   = 近 7 天 times_completed 增量最多的种
//!   * free   = 当前免费/双免促销中的种按做种数排（无促销时回退周榜）
//! 增量口径取 snatches 完成事件（created_at 是完成时刻），不是 torrents
//! 上的累计列——累计列会把三年前的老种永远钉在榜上。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct HotTorrentRow {
    id: i64,
    name: String,
    /// 体积字节数（前端格式化）
    size: i64,
    seeders: i32,
    leechers: i32,
    /// 榜值：today/week = 完成数增量；free = 做种数
    val: i64,
    rating: Option<String>,
    poster: Option<String>,
    /// 官种标记（热榜行徽标）
    official: bool,
}

const LIST_SQL_HEAD: &str = "SELECT t.id, t.name, t.size::bigint, \
    t.seeders, t.leechers, \
    NULLIF(t.media_info->>'rating','') AS rating, \
    NULLIF(t.media_info->>'poster','') AS poster, \
    t.official_tag AS official, ";

/// 三榜共用可见性谓词：过审 + 有活种。主列表默认视图同样隐藏死种
/// （view_scope / list_noclamp_as 的 `t.seeders > 0`）——榜单里挂一颗
/// 0 做种的死种，点进去只有「无种子」。
const VIS: &str =
    "FROM torrents t WHERE t.approval_status = 1 AND t.seeders > 0";

/// 今日完成增量（站点日 UTC+8，与首页 resource_stats 同口径）。
const TODAY_VAL: &str = "(SELECT count(*) FROM snatches s \
    WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL \
      AND ((s.completed_at AT TIME ZONE 'UTC') + interval '8 hours')::date \
        = ((now() AT TIME ZONE 'UTC') + interval '8 hours')::date)";

/// 近 7 天完成增量。
const WEEK_VAL: &str = "(SELECT count(*) FROM snatches s \
    WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL \
      AND s.completed_at > now() - interval '7 days')";

/// 内容热榜（P1-1）。登录可见（与 /top/boards 同口径：前台 /top 不在
/// 匿名白名单，API 必须同一策略）。
#[utoipa::path(
    get,
    path = "/api/v1/top/torrents",
    tag = "内容/榜单",
    description = "内容热榜三件套：today=今日完成增量榜 / week=近7天完成增量榜 / free=当前免费中按做种数。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/top/torrents")]
pub(super) async fn top_torrents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;

    // 今日/本周完成增量：snatches.completed_at 在窗口内的完成事件数。
    // 站点日 UTC+8 与首页 resource_stats 同口径（P-465 教训：按 UTC 分桶
    // 会让北京 0-8 点的「今日」恒空）。外层 `val > 0` 必不可少——此前无此
    // 过滤时，今天没人完成下载，榜单会拿 val=0 的老种（最低 id）填满。
    let today_q = format!(
        "SELECT * FROM ({}{} AS val {}) q \
         WHERE q.val > 0 ORDER BY q.val DESC, q.id LIMIT 10",
        LIST_SQL_HEAD, TODAY_VAL, VIS
    );
    let week_q = format!(
        "SELECT * FROM ({}{} AS val {}) q \
         WHERE q.val > 0 ORDER BY q.val DESC, q.id LIMIT 10",
        LIST_SQL_HEAD, WEEK_VAL, VIS
    );
    // 免费榜：当前 free/x2free 窗口内（站点级或本种），按做种数排；
    // 口径与 hr_snapshot 免费豁免同一谓词形状。seeders 是 INT4——
    // 三榜共用 struct 的 val 按 i64 解码，必须 ::bigint（INT4/INT8 系坑）。
    let free_q = format!(
        "{} t.seeders::bigint AS val {} \
         AND EXISTS (SELECT 1 FROM promotions p \
             WHERE (p.torrent_id = t.id OR (p.torrent_id IS NULL AND ( \
                     p.scope = 'global' \
                     OR (p.scope = 'official' AND t.official_tag) \
                     OR (p.scope = 'non_official' AND NOT t.official_tag) \
                     OR (p.scope = 'category' \
                         AND t.category_id = p.category_id))) \
               AND p.starts_at <= now() AND p.ends_at > now() \
               AND p.kind IN ('free','x2free'))) \
         ORDER BY val DESC, t.id LIMIT 10",
        LIST_SQL_HEAD, VIS
    );

    let mut out = serde_json::Map::new();
    for (key, q) in [("today", today_q), ("week", week_q), ("free", free_q)] {
        let rows: Vec<HotTorrentRow> =
            sqlx::query_as(&q)
                .fetch_all(db)
                .await
                .map_err(|e| crate::errors::DomainError::Internal(e.into()))?;
        out.insert(
            key.to_string(),
            serde_json::to_value(rows).unwrap_or_default(),
        );
    }
    Ok(ok(serde_json::Value::Object(out)))
}
