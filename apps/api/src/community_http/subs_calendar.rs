//! 追更中心（0328 movie/anime 剧集批）：我的订阅 + 追更日历。
//!
//! 现状拼图：订阅写入（group_subscribe）、过审通知（review_side_effects
//! 站内信）、group_info 组内并列——都各自在，但用户侧没有一张
//! 「我在追什么、追到哪一话/第几集、最近更新了什么」的聚合视图。
//! 本批补两个读口：
//!   · GET /me/subscriptions/groups：我的订阅列表，每行带最新话数/集数
//!     （ep_last 最大值，按站型维度自动生效——anime 用话数、movie 用集数）
//!     与最近一次过审时间；
//!   · GET /me/subscriptions/calendar：追更日历——按天分组列订阅组的
//!     新种过审事件（近 14 天），对应「周三晚更新」的追更体感。
//! 写路径零新增：订阅/通知/过审链路全部复用既有件。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;

/// 我的订阅列表（带追更进度）。
#[get("/me/subscriptions/groups")]
pub async fn my_group_subscriptions(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 追更进度 = 组内过审种的 ep_last 最大值（number 维度）；
    let rows: Vec<(i64, String, Option<i64>, Option<String>, i64)> =
        sqlx::query_as(
            r#"SELECT g.id, g.name,
               (SELECT max((ts.value #>> '{}')::numeric)::bigint
                  FROM torrents t JOIN torrent_sections ts
                    ON ts.torrent_id = t.id AND ts.kind = 'ep_last'
                 WHERE t.group_id = g.id AND t.approval_status = 1),
               (SELECT to_char(max(t.approved_at), 'YYYY-MM-DD HH24:MI')
                  FROM torrents t WHERE t.group_id = g.id
                    AND t.approval_status = 1),
               (SELECT count(*) FROM torrents t
                 WHERE t.group_id = g.id AND t.approval_status = 1)
               FROM group_subscriptions gs
               JOIN torrent_groups g ON g.id = gs.group_id
               WHERE gs.user_id = $1
               ORDER BY 4 DESC NULLS LAST, g.name"#,
        )
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
    Ok(ok(rows
        .iter()
        .map(|(gid, name, ep, last, n)| {
            serde_json::json!({
                "group_id": gid, "name": name,
                "latest_episode": ep, "last_update": last,
                "releases": n,
            })
        })
        .collect::<Vec<_>>()))
}

/// 追更日历：近 14 天内我订阅的组的新种过审事件，按天分组。
#[get("/me/subscriptions/calendar")]
pub async fn my_subscription_calendar(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(String, i64, String, i64, Option<i64>)> = sqlx::query_as(
        r#"SELECT to_char(t.approved_at, 'YYYY-MM-DD') AS day,
                  t.id, t.name, g.id,
                  (SELECT max((ts.value #>> '{}')::numeric)::bigint
                     FROM torrent_sections ts
                    WHERE ts.torrent_id = t.id AND ts.kind = 'ep_last')
           FROM torrents t
           JOIN torrent_groups g ON g.id = t.group_id
           JOIN group_subscriptions gs ON gs.group_id = g.id
           WHERE gs.user_id = $1
             AND t.approval_status = 1
             AND t.approved_at >= now() - interval '14 days'
           ORDER BY t.approved_at DESC LIMIT 200"#,
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    use std::collections::BTreeMap;
    let mut days: BTreeMap<String, Vec<serde_json::Value>> =
        BTreeMap::new();
    for (day, tid, name, gid, ep) in rows {
        days.entry(day).or_default().push(serde_json::json!({
            "torrent_id": tid, "name": name,
            "group_id": gid, "episode": ep,
        }));
    }
    // 新日期在前（BTreeMap 升序 → 反转收集）
    Ok(ok(days
        .into_iter()
        .rev()
        .map(|(day, items)| {
            serde_json::json!({ "date": day, "items": items })
        })
        .collect::<Vec<_>>()))
}
