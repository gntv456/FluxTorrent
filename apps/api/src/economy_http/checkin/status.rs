//! GET /attendance 状态口（0500 从 checkin.rs 拆出——行数门禁）：
//! 连签基线/补签卡/规则透出/热力足迹（0498）。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/attendance")]
pub(super) async fn checkin_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        // recent 列表含补签行（日历要展示）；streak 基线另查最后一条记录（补签也参与
        // 续签，§9.3 修订），见下方 last_live。
        "SELECT date, streak, \
         reward FROM attendance WHERE user_id = $1 \
            AND date >= current_date - interval '30 days' ORDER BY date",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 站点时区 UTC+8
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let checked_today = rows.iter().any(|(d, _, _)| *d == today);
    // streak 基线取「最后一条记录」——补签行也参与续签（§9.3 修订，与签到主流程
    // 同口径）。补签行 streak 已由历史重算填成真实连续值，不再作废。
    // P-471：断一天起展示必须归 0——旧写法把库里那格的旧计数原样端出去，
    // 会员首页看到「连签 12 天」、点下去回的却是 streak=1。
    let last_live: Option<(chrono::NaiveDate, i32)> = sqlx::query_as(
        "SELECT date, streak FROM attendance WHERE user_id = $1 \
         ORDER BY date DESC LIMIT 1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let current_streak: i64 = crate::economy_checkin::shown_streak(
        last_live.map(|(d, s)| (d, s as i64)),
        today,
    );
    // 补签卡持有数（双源 0262：未消耗订单 + 娱乐屋发放的补签卡；kind 兼容 makeup_card/resub_card，0066）
    let makeup_cards: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM shop_orders o \
                   JOIN shop_items i ON i.id = o.item_id \
                  WHERE o.user_id = $1 AND o.fulfill_state = 'done' \
                    AND i.kind IN ('makeup_card','resub_card') \
                    AND NOT EXISTS (SELECT 1 FROM resub_uses r \
                                     WHERE r.idempotency_key \
                     = concat('resub:', o.id))) \
             + (SELECT COALESCE(SUM(g.qty), 0)::bigint \
                  FROM arcade_item_grants g \
                 WHERE g.user_id = $1 AND g.item_key = 'resub_card') \
             - (SELECT COALESCE(SUM(u.qty), 0)::bigint \
                  FROM arcade_item_uses u \
                 WHERE u.user_id = $1 AND u.item_key = 'resub_card')",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 签到规则透出（2026-11-10）：签到页要展示「签一次得多少/里程碑哪天加送多少」，
    // 与 checkin 主流程同一套 CheckInParams（同键同回落），站长改配置页面即跟随。
    // SQL 走 repo/settings::read_many（H 层零 SQL 棘轮：本次改动不得净增）。
    let param_rows = crate::repo::settings::read_many(
        &state.repo,
        &[
            "attendance_first",
            "attendance_streak",
            "attendance_daily_cap",
            "attendance_continuous_day",
            "attendance_continuous_value",
        ],
    )
    .await
    .unwrap_or_default();
    let params = crate::economy_checkin::CheckInParams::from_settings(
        &param_rows
            .into_iter()
            .map(|(k, v, _)| (k, v))
            .collect::<Vec<_>>(),
    );
    // 周期视角（2026-11-10 循环制）：绝对连签 → 周期内位置与下一档。
    // streak=483 的人 cycle_pos=3、next_at=10（绝对 490）——规则卡据此
    // 显示「本周期第 3 天 · 下一档 10 天」，而不是永远停在「已过 30」。
    let cycle = params.cycle_days();
    let pos = params.cycle_pos(current_streak);
    let next_in_cycle = params
        .milestones
        .iter()
        .map(|(d, _)| *d)
        .filter(|d| *d > pos)
        .min();
    // 0498 签到热力足迹（近 12 个月逐月天数）：与 home 板块
    // home_attendance.rs 同款分桶，但 GET /attendance 独立成端——签到页
    // 挂热力图不必拉整页 home 数据（产品体检 A1：heatmap 已在 home 下发
    // 却无 UI 消费；本端补齐后签到页成为消费方）。
    let att_all: Vec<(chrono::NaiveDate,)> = sqlx::query_as(
        "SELECT date FROM attendance WHERE user_id = $1 \
         AND date >= current_date - interval '365 days' ORDER BY date",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let mut heat: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    for (d,) in &att_all {
        *heat.entry(d.format("%Y-%m").to_string()).or_insert(0) += 1;
    }
    let heatmap: Vec<serde_json::Value> = heat
        .into_iter()
        .map(|(m, n)| serde_json::json!({ "month": m, "days": n }))
        .collect();
    Ok(ok(serde_json::json!({
        "checked_today": checked_today, "streak": current_streak,
        "recent": rows, "makeup_cards": makeup_cards,
        "heatmap": heatmap,
        "rules": {
            "first": params.first, "streak": params.streak,
            "daily_cap": params.daily_cap,
            "milestones": params.milestones,
            // 周期制：满 cycle_days 次日归 1 重计，加送循环发
            "cycle_days": cycle,
            "cycle_pos": pos,
            "next_milestone_in_cycle": next_in_cycle,
        },
    })))
}
