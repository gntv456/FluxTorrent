//! 生态兼容层 ②a：NexusPHP 口径的用户资料 JSON 构造。
//!
//! 从 `nexusphp.rs` 拆出：`user.json` 与别名 `userdetails.php` 共用这份
//! 字段契约，单独成文件既避免两条路径口径漂移，也让主列表文件保持可拆。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// NP userdetails 口径的用户 JSON（`user.json` 与别名 `userdetails.php`
/// 共用，保证两条路径的字段口径不可能漂移）。
pub(super) async fn np_user_json(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
) -> DomainResult<serde_json::Value> {
    // 补齐做种数 / 做种体积 / 加入时间。前两项与 PTPP 端点同源
    // （snatches.seeding + users.seeding_size，后者由 worker jobs/seeding.rs
    // 维护），别在这里另算一套口径，否则两个兼容端点会漂移。
    let row: Option<(
        i64,
        String,
        i64,
        i64,
        i64,
        i32,
        i64,
        chrono::DateTime<chrono::Utc>,
        i64,
    )> = sqlx::query_as(
        "SELECT u.id, u.username, u.uploaded, u.downloaded, \
         COALESCE(u.spark_balance,0), u.class_id, \
         COALESCE(u.seeding_size,0), u.created_at, \
         (SELECT count(*) FROM snatches s \
           WHERE s.user_id = u.id AND s.seeding) \
         FROM users u WHERE u.id = $1 AND u.status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        id,
        username,
        uploaded,
        downloaded,
        seedbonus,
        class_id,
        seeding_size,
        join_at,
        seeding,
    )) = row
    else {
        return Err(DomainError::Unauthorized);
    };
    // 分享率：downloaded=0 时回 -1.0 会被工具当负数显示（0267）——
    // 改恒非负 + 语义标记：真值给数值；只上传给 ∞；皆 0 给 "---"。
    let (ratio, ratio_infinite, ratio_display) = if downloaded > 0 {
        let r =
            (uploaded as f64 / downloaded as f64 * 10000.0).round() / 10000.0;
        (r, false, format!("{r:.4}"))
    } else if uploaded > 0 {
        (0.0, true, "∞".to_string())
    } else {
        (0.0, false, "---".to_string())
    };
    // 等级：NP 口径给可读等级名（class 数字对工具没有展示价值）
    let class_name: String =
        sqlx::query_scalar("SELECT name FROM user_classes WHERE id = $1")
            .bind(class_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .unwrap_or_else(|| format!("LV{class_id}"));
    let join_str = join_at.to_rfc3339();
    Ok(serde_json::json!({
        "id": id,
        "username": username,
        "uploaded": uploaded,
        "downloaded": downloaded,
        "seedbonus": seedbonus,
        "class": class_id,
        "class_name": class_name,
        "ratio": ratio,
        "ratio_infinite": ratio_infinite,
        "ratio_display": ratio_display,
        // 做种与注册时间。蛇形为主，同时给 PTPP 口径的驼峰别名，
        // 让只认 userdetails.php 老字段的工具也能直接读。
        "seeding": seeding,
        "seeding_size": seeding_size,
        "seedingSize": seeding_size,
        "join_at": join_str.as_str(),
        "joinTime": join_str.as_str(),
    }))
}
