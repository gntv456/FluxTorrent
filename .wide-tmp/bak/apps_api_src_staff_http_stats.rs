//! 统计/保种/清理。
//! 从 staff_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 统计（stats.php 口径）：服务器/站点核心数据
#[get("/admin/stats")]
pub async fn admin_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STATS_VIEW)
        .await?;
    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users WHERE status < 2)::bigint, \
                (SELECT count(*) FROM torrents)::bigint, \
                (SELECT count(*) FROM snatches WHERE seeding)::bigint, \
                (SELECT count(*) FROM snatches WHERE leeching)::bigint, \
                (SELECT count(*) FROM comments)::bigint, \
                (SELECT count(*) FROM messages)::bigint",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (users, torrents_n, seeding_n, leeching_n, comments_n, messages_n) =
        row;
    let redis_ok = {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let _: Option<i64> = c.get("flux:ping").await.ok().flatten().or(None);
        true
    };
    Ok(ok(serde_json::json!({
        "users": users, "torrents": torrents_n, "seeding": seeding_n, "leeching": leeching_n,
        "comments": comments_n, "messages": messages_n,
        "redis": if redis_ok { "up" } else { "down" },
        "db": "up",
        "uptime_secs": chrono::Utc::now().timestamp() - state.started_at.timestamp(),
    })))
}

/// 清除缓存（clearcache.php 口径）：Redis 前缀清理
/// 保种统计（seed.stats.view）：站点做种总览与 Top 保种用户。
/// 注意：不走 staff 门槛——保种员 / VIP 持该权限即可访问（非管理组角色）。
#[get("/seed-stats")]
pub async fn seed_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SEED_STATS_VIEW,
    )
    .await?;
    let totals: (i64, i64, f64) = sqlx::query_as(
        "SELECT count(DISTINCT s.user_id)::bigint, count(*)::bigint, \
                COALESCE(avg(s.seeded_seconds) / 3600.0, 0)::float8 \
         FROM snatches s WHERE s.seeding",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_count: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT s.user_id, u.username, count(*)::bigint AS c \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.seeding GROUP BY s.user_id, u.username ORDER BY c DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_hours: Vec<(i64, String, f64)> = sqlx::query_as(
        "SELECT s.user_id, u.username, (sum(s.seeded_seconds) / 3600.0)::float8 AS h \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.seeded_seconds > 0 GROUP BY s.user_id, u.username ORDER BY h DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_count = top_count
        .into_iter()
        .map(|(uid, name, c)| serde_json::json!({"user_id": uid, "username": name, "seeding": c}))
        .collect::<Vec<_>>();
    let top_hours = top_hours
        .into_iter()
        .map(|(uid, name, hrs)| serde_json::json!({"user_id": uid, "username": name, "hours": (hrs * 10.0).round() / 10.0}))
        .collect::<Vec<_>>();
    Ok(ok(serde_json::json!({
        "seeders": totals.0,
        "seeding_torrents": totals.1,
        "avg_seed_hours": (totals.2 * 10.0).round() / 10.0,
        "top_by_count": top_count,
        "top_by_hours": top_hours,
    })))
}

#[post("/admin/clearcache")]
pub async fn clear_cache(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEARCACHE)
        .await?;
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let keys: Vec<String> = c.keys("rl:*").await.unwrap_or_default();
    let n = keys.len();
    if n > 0 {
        let _: () = redis::cmd("DEL")
            .arg(&keys)
            .query_async(&mut c)
            .await
            .unwrap_or(());
    }
    state.repo.audit(Some(auth.id), "clear_cache", None).await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

/// 做清理（docleanup.php 口径）：过期促销/过期警告/过期登录事件归档清理
#[post("/admin/docleanup")]
pub async fn do_cleanup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    // 审计修复（P0）：旧版删 7 天前过期促销，而 worker expire_promotions 保留 365 天——
    // hr_enforce 建快照需按 completed_at 时点回查当时促销，物理删掉近期历史会让
    // H&R 豁免回查失明（免费期完成的下载被误判违规）。与 worker 统一为 365 天。
    let expired_promos = sqlx::query(
        "DELETE FROM promotions WHERE ends_at < now() - interval '365 days'",
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    let expired_warns = sqlx::query(
        "UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE warned_until IS NOT NULL AND warned_until < now()",
    ).execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    let old_logins =
        sqlx::query("DELETE FROM login_events WHERE created_at < now() - interval '90 days'")
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    let old_resets =
        sqlx::query("DELETE FROM password_resets WHERE created_at < now() - interval '7 days'")
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    state.repo.audit(Some(auth.id), "do_cleanup", None).await;
    Ok(ok(serde_json::json!({
        "expired_promotions": expired_promos,
        "expired_warnings": expired_warns,
        "old_login_events": old_logins,
        "old_password_resets": old_resets,
    })))
}
