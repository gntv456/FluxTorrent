//! me 总览与设置（M01）：overview + settings get/put。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, Responder};
use sqlx::Row;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/me/overview")]
pub async fn me_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    // 主档 + 统计（一次查询，口径与旧站 my_data_stats 一致）
    let row = sqlx::query(
        r#"
        SELECT u.username, u.email, u.uploaded, u.downloaded, u.created_at, u.avatar_url,
               u.parked, u.privacy, u.totp_enabled, u.passkey,
               u.spark_balance, f.css AS avatar_frame_css, f.image_url AS avatar_frame_image,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.seeding) AS seeding,
               (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.leeching) AS leeching,
               (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS uploads,
               (SELECT count(*) FROM comments c WHERE c.user_id = u.id) AS comments,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 0) AS invites_pending,
               (SELECT count(*) FROM invites i WHERE i.inviter_id = u.id AND i.status = 1) AS invites_used,
               (SELECT count(*) FROM user_medals m WHERE m.user_id = u.id) AS medals,
               c.name AS class_name, c.id AS cid
        FROM users u LEFT JOIN user_classes c ON c.id = u.class_id
             LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id
        WHERE u.id = $1
        "#,
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::Unauthorized)?;
    // 佩戴勋章（userbar 同口径，控制面板资料卡用户名角标）
    let worn_medals: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT m.name, m.asset_ref FROM user_medals um JOIN medals m ON m.id = um.medal_id \
         WHERE um.user_id = $1 AND um.wearing AND (um.expires_at IS NULL OR um.expires_at > now()) \
         ORDER BY m.id LIMIT 3",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let r = &row;
    let get = |col: &str| -> serde_json::Value {
        r.try_get(col).unwrap_or(serde_json::Value::Null)
    };
    let get_opt_str = |col: &str| -> Option<String> {
        r.try_get::<Option<String>, _>(col).ok().flatten()
    };
    let get_i64 = |col: &str| -> i64 { r.try_get::<i64, _>(col).unwrap_or(0) };
    let get_bool =
        |col: &str| -> bool { r.try_get::<bool, _>(col).unwrap_or(false) };
    let get_str = |col: &str| -> String {
        r.try_get::<String, _>(col).unwrap_or_default()
    };
    let get_ts = |col: &str| -> Option<String> {
        r.try_get::<chrono::DateTime<chrono::Utc>, _>(col)
            .ok()
            .map(|t| t.to_rfc3339())
    };

    let downloaded = get_i64("downloaded");
    let uploaded = get_i64("uploaded");
    let ratio = if downloaded == 0 {
        None
    } else {
        Some((uploaded as f64 / downloaded as f64 * 100.0).round() / 100.0)
    };
    // 最近 30 天登录趋势（对齐旧站 usercp 首页活跃图）
    let trend: Vec<(chrono::NaiveDate, i64)> = sqlx::query_as(
        "SELECT created_at::date AS d, count(*) AS n FROM login_events \
         WHERE user_id = $1 AND created_at > now() - interval '30 days' \
         GROUP BY d ORDER BY d",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(d, n)| (d, n))
    .collect::<Vec<_>>();
    let trend_json: Vec<serde_json::Value> = trend
        .iter()
        .map(|(d, n)| serde_json::json!({ "date": d.format("%Y-%m-%d").to_string(), "count": n }))
        .collect();
    let login_total_30d: i64 = trend.iter().map(|(_, n)| n).sum();
    let last_login: Option<String> = sqlx::query_scalar(
        "SELECT max(created_at)::text FROM login_events WHERE user_id = $1",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 等级进度：下一等级阈值以 class_rules 为准（worker class_auto_adjust 的升降级权威来源；
    // user_classes.min_uploaded 在 live 数据中全为 0，用它预览会恒显示「已达」）
    let class_id = get_i64("cid") as i32;
    let next: Option<(String, i64)> = sqlx::query_as(
        "SELECT name, min_uploaded FROM class_rules \
         WHERE class_id > $1 AND class_id < 90 AND min_uploaded > 0 \
         ORDER BY class_id LIMIT 1",
    )
    .bind(class_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let seeding = get_i64("seeding");
    let seed_points = (seeding as f64) * 100.0;
    let (next_name, next_req) =
        next.unwrap_or_else(|| ("Max".into(), uploaded.max(1)));

    Ok(ok(serde_json::json!({
        "id": uid,
        "username": get_str("username"),
        "email": get_str("email"),
        "class_name": get("class_name"),
        "avatar_url": get_opt_str("avatar_url"),
        "avatar_frame_css": get_opt_str("avatar_frame_css"),
        "avatar_frame_image": get_opt_str("avatar_frame_image"),
        "worn_medals": worn_medals
            .into_iter()
            .map(|(name, asset_ref)| serde_json::json!({ "name": name, "asset_ref": asset_ref }))
            .collect::<Vec<_>>(),
        "created_at": get_ts("created_at"),
        "uploaded": uploaded,
        "downloaded": downloaded,
        "ratio": ratio,
        "seeding": seeding,
        "leeching": get_i64("leeching"),
        "uploads": get_i64("uploads"),
        "comments": get_i64("comments"),
        "bookmarks": 0,
        "spark_balance": get_i64("spark_balance"),
        "invites_pending": get_i64("invites_pending"),
        "invites_used": get_i64("invites_used"),
        "medals": get_i64("medals"),
        "parked": get_bool("parked"),
        "privacy": get_str("privacy"),
        "totp_enabled": get_bool("totp_enabled"),
        "passkey": get_str("passkey"),
        "last_ip": "—",
        "login_trend_30d": trend_json,
        "login_days_30d": trend.iter().filter(|(_, n)| *n > 0).count(),
        "login_total_30d": login_total_30d,
        "last_login": last_login,
        "seed_points": seed_points,
        "next_class": { "name": next_name, "required": next_req },
    })))
}
