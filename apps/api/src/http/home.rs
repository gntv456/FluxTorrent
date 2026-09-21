//! 首页五大板块 + 布局。
//! 从 http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;

// ============ 首页（复刻包子站 index.php 五大板块） ============

/// 首页汇总：公告 + 签到日历 + 30 天新增资源统计 + 站点数据 + 娱乐流水
#[get("/home")]
pub async fn home_sections(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;

    // 公告（home-news）：最新一条为头条 + 其余为列表
    let news: Vec<(
        i32,
        String,
        String,
        String,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, title, body, badge, \
         created_at FROM announcements ORDER BY id DESC LIMIT 8",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let news_json: Vec<serde_json::Value> = news
        .iter()
        .map(|(id, title, body, badge, ts)| {
            // 公告 body 为富文本 HTML（管理员撰写，NP 口径）：出站前 ammonia 白名单消毒，
            // 剥离 script/事件属性/javascript: 协议 —— 管理员账号被盗也不构成全站存储 XSS
            let safe_body = ammonia::Builder::default().clean(body).to_string();
            serde_json::json!({
                "id": id, "title": title, "body": safe_body, "badge": badge,
                "date": ts.format("%m-%d").to_string(),
            })
        })
        .collect();

    // 签到日历（attendance-card）：当月逐日 + 连签/累计 + 补签卡持有数（0066）
    let makeup_cards: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let att: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        "SELECT date, streak, \
         reward FROM attendance WHERE user_id = $1 ORDER BY date",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
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

    // 30 天新增资源统计（home-resource-stats）：普通 vs 官种
    let daily: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT created_at::date AS d, \
                count(*) FILTER (WHERE NOT official_tag) AS ordinary, \
                count(*) FILTER (WHERE official_tag) AS official \
         FROM torrents WHERE approval_status = 1 AND created_at > now() - interval '30 days' \
         GROUP BY d ORDER BY d",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
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
    let (
        users,
        torrents_n,
        peers,
        seeders,
        leechers,
        warned,
        banned,
        unverified,
    ): (i64, i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM users WHERE status < 2), \
            (SELECT count(*) FROM torrents WHERE approval_status = 1), \
            (SELECT count(*) FROM snatches WHERE seeding OR leeching), \
            (SELECT count(*) FROM snatches WHERE seeding), \
            (SELECT count(*) FROM snatches WHERE leeching), \
            (SELECT count(*) FROM users WHERE status = 1), \
            (SELECT count(*) FROM users WHERE status >= 2), \
            (SELECT count(*) FROM users WHERE must_reset_password)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up_sum, down_sum, size_sum): (i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT COALESCE(sum(uploaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(downloaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(size),0)::bigint FROM torrents WHERE approval_status = 1)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 娱乐流水（幸运大转盘 → 以 spark_ledger 游戏类流水近似）
    let lucky: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT u.username, l.kind, l.amount FROM spark_ledger l \
         JOIN users u ON u.id = l.user_id \
         WHERE l.kind LIKE '%game%' OR l.kind LIKE '%vote%' \
         ORDER BY l.created_at DESC LIMIT 15",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 友情链接
    let links: Vec<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT name, url, title FROM friend_links ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "news": news_json,
        "attendance": {
            "month": today.format("%Y年%m月").to_string(),
            "streak": streak, "total_days": total_days, "checked_today": checked_today,
            "calendar": calendar, "makeup_cards": makeup_cards,
        },
        "resource_stats": {
            "today": today_count, "avg7": (avg7 * 10.0).round() / 10.0,
            "total30": total30, "series": series,
        },
        "site_data": {
            "users": users, "torrents": torrents_n, "peers": peers,
            "seeders": seeders, "leechers": leechers,
            "warned": warned, "banned": banned, "unverified": unverified,
            "total_upload": up_sum, "total_download": down_sum, "total_size": size_sum,
        },
        "lucky_draw": lucky.iter().map(|(u, k, a)| serde_json::json!({
            "user": u, "kind": k, "amount": a,
        })).collect::<Vec<_>>(),
        "friend_links": links.iter().map(|(n, u, t)| serde_json::json!({
            "name": n, "url": u, "title": t,
        })).collect::<Vec<_>>(),
        // 首页排版（0089）：site_settings.home_layout 原样透传（JSON 数组或空串），
        // 前端空/非法回退默认布局
        "home_layout": crate::http::home_layout_raw(&state.repo.db).await,
    })))
}
