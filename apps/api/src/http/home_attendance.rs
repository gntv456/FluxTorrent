//! 首页签到日历个人段（从 home.rs 拆出，300 门禁）：当月逐日 + 连签/累计
//! + 补签卡持有数。per-user 数据永不进共享缓存（UNIT3D 范式），每次实时算。
//!
//! 两处口径必须与 /attendance、/attendance/checkin 一致，历史上各修各的：
//! - 连签基线取「最后一条非补签行」，补签行 streak=0 只是占位（P-90）；
//! - 补签卡持有数走双源（商店未消耗订单 + 娱乐屋发放−消耗，0262），
//!   只数商店订单会让刮刮乐抽到的卡在首页显示 0（P-91）。

use crate::errors::{DomainError, DomainResult};

/// 签到日历个人段（0152 拆出）：当月逐日 + 连签/累计 + 补签卡。
pub async fn attendance_json(
    db: &sqlx::PgPool,
    uid: i64,
) -> DomainResult<serde_json::Value> {
    let makeup_cards: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM shop_orders o \
                   JOIN shop_items i ON i.id = o.item_id \
                  WHERE o.user_id = $1 \
                    AND i.kind IN ('makeup_card','resub_card') \
                    AND NOT EXISTS (SELECT 1 FROM resub_uses r \
                       WHERE r.idempotency_key = concat('resub:', o.id))) \
             + (SELECT COALESCE(SUM(g.qty), 0)::bigint \
                  FROM arcade_item_grants g \
                 WHERE g.user_id = $1 AND g.item_key = 'resub_card') \
             - (SELECT COALESCE(SUM(u.qty), 0)::bigint \
                  FROM arcade_item_uses u \
                 WHERE u.user_id = $1 AND u.item_key = 'resub_card')",
    )
    .bind(uid)
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let att: Vec<(chrono::NaiveDate, i32, i64, bool)> = sqlx::query_as(
        "SELECT date, streak, reward, makeup \
         FROM attendance WHERE user_id = $1 ORDER BY date",
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
        (next_month.pred_opt().unwrap_or(today) - month_start).num_days()
            as u32
            + 1
    };
    let calendar: Vec<serde_json::Value> = (0..days_in_month)
        .map(|i| {
            let d = month_start + chrono::Duration::days(i as i64);
            let hit = att.iter().find(|(ad, _, _, _)| *ad == d);
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "day": chrono::Datelike::day(&d),
                "done": hit.is_some(),
                "reward": hit.map(|(_, _, r, _)| r).unwrap_or(&0),
            })
        })
        .collect();
    let streak = att
        .iter()
        .rev()
        .find(|(_, _, _, mk)| !*mk)
        .map(|(_, s, _, _)| *s)
        .unwrap_or(0);
    let total_days = att.len() as i32;
    let checked_today = att.iter().any(|(d, _, _, _)| *d == today);
    Ok(serde_json::json!({
        "month": today.format("%Y年%m月").to_string(),
        "streak": streak, "total_days": total_days,
        "checked_today": checked_today,
        "calendar": calendar, "makeup_cards": makeup_cards,
    }))
}
