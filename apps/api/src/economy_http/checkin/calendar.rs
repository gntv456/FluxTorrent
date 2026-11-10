//! 签到日历（按月翻月查询）——从 checkin.rs 按 300 行门禁拆出。
//! 分层登记：本文件带 1 处 sqlx（layer_baseline 已收录）——存量拆件挪存，
//! 非新增 SQL 面；净 SQL 化按维护节奏随 repo/ 层下沉后 --update 收缩。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, Responder};

/// 按月查签到日历（翻月用）：?ym=YYYY-MM，返回该用户该月逐日签到状态。
/// 桌面签到卡与手机万年历翻月都调这个口，历史月份的签到标记才看得见。
/// 缺省/非法 ym → 当月。makeup 补签行也计为已签（done=true）。
#[get("/attendance/calendar")]
async fn checkin_calendar(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<CalQ>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    // 解析 ym=YYYY-MM；缺省/非法回落当月
    let (y, m) = q.ym.as_deref().and_then(parse_ym).unwrap_or_else(|| {
        (
            chrono::Datelike::year(&today),
            chrono::Datelike::month(&today),
        )
    });
    let month_start = chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(today);
    let next_month = if m == 12 {
        chrono::NaiveDate::from_ymd_opt(y + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(y, m + 1, 1)
    }
    .unwrap_or(month_start);
    let days_in_month = (next_month - month_start).num_days();
    // 该月已有签到行（date → reward；补签 reward=0）
    let rows: Vec<(chrono::NaiveDate, i64, bool)> = sqlx::query_as(
        "SELECT date, reward, makeup FROM attendance \
         WHERE user_id = $1 AND date >= $2 AND date < $3 ORDER BY date",
    )
    .bind(auth.id)
    .bind(month_start)
    .bind(next_month)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let by_date: std::collections::HashMap<chrono::NaiveDate, (i64, bool)> =
        rows.into_iter().map(|(d, r, mk)| (d, (r, mk))).collect();
    let days: Vec<serde_json::Value> = (0..days_in_month)
        .map(|i| {
            let d = month_start + chrono::Duration::days(i);
            let (reward, makeup) =
                by_date.get(&d).copied().unwrap_or((0, false));
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "day": chrono::Datelike::day(&d),
                "done": by_date.contains_key(&d),
                "reward": reward,
                "makeup": makeup,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "ym": format!("{y:04}-{m:02}"), "days": days,
    })))
}

#[derive(serde::Deserialize)]
struct CalQ {
    #[serde(default)]
    ym: Option<String>,
}

/// "YYYY-MM" → (year, month)；非法返回 None
fn parse_ym(s: &str) -> Option<(i32, u32)> {
    let (ys, ms) = s.split_once('-')?;
    let y: i32 = ys.parse().ok()?;
    let m: u32 = ms.parse().ok()?;
    if (1..=12).contains(&m) && (1900..=9999).contains(&y) {
        Some((y, m))
    } else {
        None
    }
}
