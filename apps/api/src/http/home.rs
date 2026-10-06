//! 首页五大板块 + 布局。
//! 从 http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;

// ============ 首页（复刻包子站 index.php 五大板块） ============

use redis::AsyncCommands;

/// 首页共享段 Redis 缓存键（0152）：公告/资源统计/站点数据/娱乐流水/友链/排版。
/// 写失效式（Gazelle 范式）：管理端 news/links/home_layout/promos 写操作 DEL 本键，
/// TTL 300s 兜底覆盖种子发布（资源统计）与游戏流水这类无写入口的慢变化。
pub const HOME_SHARED_CACHE_KEY: &str = "cache:home:shared:v2";
// v1→v2：本批 payload 新增 home_sections（板块单源清单）。前端拿到缺该字段的
// 旧缓存会把首页当「无板块」渲染，直接换 key 让旧条目一次性作废。

/// 供管理端写路径主动失效（写失效式缓存，Gazelle 范式）
pub async fn invalidate_home_cache(state: &AppState) {
    let mut c = state.redis.clone();
    let _: Result<(), _> = c.del(HOME_SHARED_CACHE_KEY).await;
}

/// 首页汇总：共享段（缓存 300s + 写失效）+ 个人段（签到日历，逐用户实时）。
/// 首页对比调研 0152：此前每次访问打 10+ 条实时 SQL，是全参评项目中唯一零缓存的首页；
/// 拆分口径对齐 UNIT3D「凡带 per-user 态的查询绕开共享缓存」——签到日历/补签卡
/// 留在个人段，其余六段进共享缓存。Redis 故障 fail-open 直查库。
#[get("/home")]
pub async fn home_sections(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;

    // ---- 共享段：读缓存（fail-open）----
    let mut c = state.redis.clone();
    let hit: Option<String> = AsyncCommands::get(&mut c, HOME_SHARED_CACHE_KEY)
        .await
        .unwrap_or(None);
    let shared: serde_json::Value = match hit
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    {
        Some(v) => v,
        None => {
            let v = home_shared_fresh(&state).await?;
            if let Ok(s) = serde_json::to_string(&v) {
                let _: Result<(), _> = AsyncCommands::set_ex(
                    &mut c,
                    HOME_SHARED_CACHE_KEY,
                    s,
                    300u64,
                )
                .await;
            }
            v
        }
    };

    // ---- 个人段：签到日历（per-user，永不进共享缓存）----
    // 二审 G8：attendance/games 模块关闭时不下发对应板块（前端也无入口），
    // 空数据会让关闭模块的站首页仍出现签到日历/抽奖流水。
    let attendance_on = state.require_module("attendance").await.is_ok();
    let attendance = if attendance_on {
        attendance_json(&state.repo.read_db, uid).await?
    } else {
        serde_json::json!({ "off": true })
    };

    let mut out = shared;
    out["attendance"] = attendance;
    let games_on = state.require_module("games").await.is_ok();
    if !games_on {
        out["lucky_draw"] = serde_json::json!([]);
    }
    Ok(ok(out))
}

/// 首页共享段全量重算（缓存未命中时）：公告 + 30 天资源统计 + 站点数据 + 娱乐流水 + 友链 + 排版
async fn home_shared_fresh(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<serde_json::Value> {
    let db = &state.repo.read_db;

    // 公告（home-news）：组装 + 视频白名单消毒在 home_news.rs（0190 拆出）
    let (news_json, latest_news_id) =
        super::home_news::home_news_json(db).await?;

    // 签到日历已拆至个人段 attendance_json（per-user 不进共享缓存，0066/0152）

    // 30 天新增资源统计（home-resource-stats）：普通 vs 官种
    let daily: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT created_at::date AS d, \
                count(*) FILTER (WHERE NOT official_tag) AS ordinary, \
                count(*) FILTER (WHERE official_tag) AS official \
         FROM torrents WHERE approval_status = 1 \
              AND created_at > now() - interval '30 days' \
         GROUP BY d ORDER BY d",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 站点时区 UTC+8（与 /attendance、/checkin 同口径；容器 TZ=UTC 时 Local 会让
    // 首页日历在北京 0-8 点窗口显示「昨日未签」）
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let _ = today; // 日历构造已随签到段移入 attendance_json

    let by_day: std::collections::HashMap<chrono::NaiveDate, (i64, i64)> =
        daily.iter().map(|(d, o, f)| (*d, (*o, *f))).collect();
    let series: Vec<serde_json::Value> = (0..30)
        .rev()
        .map(|i| {
            let d = today - chrono::Duration::days(i);
            let (o, f) = by_day.get(&d).cloned().unwrap_or((0, 0));
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "ordinary": o, "official": f, "total": o + f,
            })
        })
        .collect();
    let today_count = by_day.get(&today).map(|(o, f)| o + f).unwrap_or(0);
    let last7: Vec<i64> = (1..=7)
        .map(|i| {
            by_day
                .get(&(today - chrono::Duration::days(i)))
                .map(|(o, f)| o + f)
                .unwrap_or(0)
        })
        .collect();
    let avg7 = if last7.is_empty() {
        0.0
    } else {
        last7.iter().sum::<i64>() as f64 / last7.len() as f64
    };
    let total30: i64 = daily.iter().map(|(_, o, f)| o + f).sum();

    // 站点数据（home-site-data 三列）
    // 口径修正（评审 P0-1）：墓碑行（status=3 的 deleted-* 账号）是 e2e/删号清理产物，
    // 不是「被禁用户」；must_reset_password 也不等于「未验证」。两者都把墓碑
    // （must_reset_password 恒真）排除，否则空站会出现「11 人 10 被禁」的矛盾数字。
    #[rustfmt::skip]
    let (users, torrents_n, peers, seeders, leechers, warned, banned, unverified): (
        i64, i64, i64, i64, i64, i64, i64, i64,
    ) = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM users WHERE status < 2), \
            (SELECT count(*) FROM torrents WHERE approval_status = 1), \
            (SELECT count(*) FROM snatches WHERE seeding OR leeching), \
            (SELECT count(*) FROM snatches WHERE seeding), \
            (SELECT count(*) FROM snatches WHERE leeching), \
            (SELECT count(*) FROM users WHERE status = 1), \
            (SELECT count(*) FROM users WHERE status = 2), \
            (SELECT count(*) FROM users WHERE must_reset_password \
             AND status < 2 AND username NOT LIKE 'deleted-%')",
    )
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up_sum, down_sum, size_sum): (i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT COALESCE(sum(uploaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(downloaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(size),0)::bigint FROM torrents \
             WHERE approval_status = 1)",
    )
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 娱乐流水（幸运大转盘 → 以 spark_ledger 游戏类流水近似）；
    // 公开面口径与 /top/boards、娱乐屋周榜一致：墓碑号（status>=2）不进展示
    let lucky: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT u.username, l.kind, l.amount FROM spark_ledger l \
         JOIN users u ON u.id = l.user_id \
         WHERE (l.kind LIKE '%game%' OR l.kind LIKE '%vote%') \
               AND u.status < 2 \
         ORDER BY l.created_at DESC LIMIT 15",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 友情链接
    let links: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT name, url, title FROM friend_links ORDER BY sort",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(serde_json::json!({
        "news": news_json,
        "latest_news_id": latest_news_id,
        "resource_stats": {
            "today": today_count, "avg7": (avg7 * 10.0).round() / 10.0,
            "total30": total30, "series": series,
        },
        "site_data": {
            "users": users, "torrents": torrents_n, "peers": peers,
            "seeders": seeders, "leechers": leechers,
            "warned": warned, "banned": banned, "unverified": unverified,
            "total_upload": up_sum, "total_download": down_sum,
            "total_size": size_sum,
        },
        "lucky_draw": lucky.iter().map(|(u, k, a)| serde_json::json!({
            "user": u, "kind": k, "amount": a,
        })).collect::<Vec<_>>(),
        "friend_links": links.iter().map(|(n, u, t)| serde_json::json!({
            "name": n, "url": u, "title": t,
        })).collect::<Vec<_>>(),
        // 首页排版（0089）：site_settings.home_layout 原样透传（JSON 数组或空串），
        // 前端按 home_sections 清单解析（清单单源在后端，四审 L6）
        "home_layout": crate::http::home_layout_raw(db).await,
        "home_sections": crate::http::home_sections_json(),
    }))
}

/// 签到日历个人段（0152 拆出）：当月逐日 + 连签/累计 + 补签卡。
/// per-user 数据永不进共享缓存（UNIT3D 范式），每次实时算。
async fn attendance_json(
    db: &sqlx::PgPool,
    uid: i64,
) -> DomainResult<serde_json::Value> {
    let makeup_cards: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o \
         JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r \
                WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(uid)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let att: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        "SELECT date, streak, \
         reward FROM attendance WHERE user_id = $1 ORDER BY date",
    )
    .bind(uid)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 站点时区 UTC+8（与 /attendance、/checkin 同口径；容器 TZ=UTC 时 Local 会让
    // 首页日历在北京 0-8 点窗口显示「昨日未签」）
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let month_start = chrono::Datelike::with_day(&today, 1).unwrap_or(today);
    let days_in_month = {
        let next_month = if chrono::Datelike::month(&month_start) == 12 {
            chrono::NaiveDate::from_ymd_opt(
                chrono::Datelike::year(&month_start) + 1,
                1,
                1,
            )
        } else {
            chrono::NaiveDate::from_ymd_opt(
                chrono::Datelike::year(&month_start),
                chrono::Datelike::month(&month_start) + 1,
                1,
            )
        }
        .unwrap_or(today);
        (next_month.pred_opt().unwrap_or(today) - month_start).num_days() as u32
            + 1
    };
    let calendar: Vec<serde_json::Value> = (0..days_in_month)
        .map(|i| {
            let d = month_start + chrono::Duration::days(i as i64);
            let hit = att.iter().find(|(ad, _, _)| *ad == d);
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "day": chrono::Datelike::day(&d),
                "done": hit.is_some(),
                "reward": hit.map(|(_, _, r)| r).unwrap_or(&0),
            })
        })
        .collect();
    let streak = att.iter().rev().next().map(|(_, s, _)| *s).unwrap_or(0);
    let total_days = att.len() as i32;
    let checked_today = att.iter().any(|(d, _, _)| *d == today);
    Ok(serde_json::json!({
        "month": today.format("%Y年%m月").to_string(),
        "streak": streak, "total_days": total_days,
        "checked_today": checked_today,
        "calendar": calendar, "makeup_cards": makeup_cards,
    }))
}
