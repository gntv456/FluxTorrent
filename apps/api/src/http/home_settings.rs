//! 首页要用的**站点级**设置（进共享段缓存，不随人变）。
//!
//! 两个口径：
//!   * `ratio_watch_threshold` —— 分享率处置线（观察期阈值）。跌破它会进
//!     观察期，到期仍跌破才停下载（worker/jobs/audit.rs 消费）。前端仪表盘
//!     拿它告诉会员「离停下载的线还差多少」。
//!   * `attendance_continuous_day` / `attendance_continuous_value` —— 签到
//!     连签里程碑（10/20/30 天 → 200/500/1000 魔力）。首屏签到条据此显示
//!     「下一档还差几天、能拿多少」。
//!
//! 取不到就下发空/null：前端据此不画提示，而不是拿 0 兜底（0 会被读成
//! 「阈值 0 = 没有线」「里程碑 0 天」，比没有这条信息更糟）。

use serde_json::Value;

/// 分享率处置线；站点没配这一项时返回 None。
pub async fn ratio_watch_threshold(db: &sqlx::PgPool) -> Option<f64> {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'ratio_watch_threshold'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.trim().parse::<f64>().ok())
}

/// 连签里程碑：[{day, value}]，按天数列升序。
pub async fn attendance_milestones(db: &sqlx::PgPool) -> Vec<Value> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings \
         WHERE name IN ('attendance_continuous_day', \
                       'attendance_continuous_value')",
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let pick = |key: &str| -> Vec<i64> {
        rows.iter()
            .find(|(n, _)| n == key)
            .map(|(_, v)| {
                v.split(',')
                    .filter_map(|x| x.trim().parse::<i64>().ok())
                    .collect()
            })
            .unwrap_or_default()
    };
    let days = pick("attendance_continuous_day");
    let values = pick("attendance_continuous_value");
    days.iter()
        .zip(values.iter())
        .map(|(d, v)| serde_json::json!({ "day": d, "value": v }))
        .collect()
}
