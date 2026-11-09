//! 签到（M12）与状态。
//! 从 economy_http.rs 按域拆出。

use super::spend::earn_spark_tx;

#[path = "checkin/calendar.rs"]
mod calendar;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
pub(crate) use calendar::checkin_calendar;

use crate::dto::ok;
use crate::economy_checkin::checkin_reward;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

#[post("/attendance/checkin")]
async fn checkin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive(); // 站点时区 UTC+8
    let yesterday = today - chrono::Duration::days(1);

    // 幂等：今日已签直接返回
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance WHERE user_id = $1 AND \
         date = $2)",
    )
    .bind(auth.id)
    .bind(today)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if already {
        return Err(DomainError::Validation("今天已经签到过啦".into()));
    }

    let last: Option<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        // 连签基线取「最后一条记录」——**补签行（makeup）也参与续签**（§9.3 修订）：
        // 历史重算已把补签行的 streak 填成真实连续值（不是 0），补签是用户花补签卡
        // 补的漏签日，理应让连签接得上。此前排除 makeup 会让「昨天补签、今天签到」
        // 被判断链、200+ 天连签被打回 1（实测 user 10556）。
        "SELECT date, streak, (count(*) OVER ())::bigint FROM attendance \
         WHERE user_id = $1 ORDER BY date DESC LIMIT 1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let (prev_date, prev_streak, total_days): (
        Option<chrono::NaiveDate>,
        i64,
        i64,
    ) = match last {
        Some((d, s, c)) => (Some(d), s as i64, c),
        None => (None, 0, 0),
    };
    let streak = if prev_date == Some(yesterday) {
        prev_streak + 1
    } else {
        1
    };
    let reward = checkin_reward(
        streak,
        total_days == 0,
        &crate::economy_checkin::CheckInParams::from_settings(
            &sqlx::query_as::<_, (String, String)>(
                "SELECT name, value FROM site_settings WHERE name IN \
                 ('attendance_first','attendance_streak', \
                  'attendance_daily_cap','attendance_continuous_day', \
                  'attendance_continuous_value')",
            )
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default(),
        ),
    );

    // 每日签到是否送出了抽卡券（gacha daily_free；事务块内赋值）
    let mut df_granted = false;
    {
        // 单事务（P1 撕裂窗口收口）：签到行与本日奖励同生共死——旧版 attendance 落库
        // 成功后 earn 失败，当日奖励永久漏发（重试被「已签到」拦截，幂等键空有设计）。
        let mut tx = state
            .repo
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let inserted = sqlx::query(
            "INSERT INTO attendance (user_id, date, streak, reward) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, date) DO NOTHING",
        )
        .bind(auth.id)
        .bind(today)
        .bind(streak)
        .bind(reward.total)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if inserted.rows_affected() == 0 {
            return Err(DomainError::Validation("今天已经签到过啦".into()));
        }
        let idem = format!("attendance:{}:{}", auth.id, today.format("%Y%m%d"));
        // 同事务内已由 ON CONFLICT DO NOTHING 保证首签唯一，此处必为 Spent；
        // 显式丢弃以满足 must_use 契约
        let earn_outcome =
            earn_spark_tx(&mut tx, auth.id, reward.total, "attendance", &idem)
                .await?;
        let _ = earn_outcome;
        // 每日签到送 1 张抽卡券（gacha daily_free）：券走独立账本不进火花，
        // 零通胀。幂等键带日期 —— 重放 / 补签路径都不会重复发。
        let df_idem =
            format!("gacha:df:{}:{}", auth.id, today.format("%Y%m%d"));
        let df_seen: Option<i32> = sqlx::query_scalar(
            "SELECT balance_after FROM gacha_ticket_ledger \
             WHERE idempotency_key = $1",
        )
        .bind(&df_idem)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if df_seen.is_none() {
            sqlx::query(
                "INSERT INTO gacha_ticket_balance (user_id, balance) \
                 VALUES ($1, 0) ON CONFLICT (user_id) DO NOTHING",
            )
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let tb: i32 = sqlx::query_scalar(
                "SELECT balance FROM gacha_ticket_balance \
                 WHERE user_id = $1 FOR UPDATE",
            )
            .bind(auth.id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "INSERT INTO gacha_ticket_ledger (user_id, delta, kind, \
                 ref_type, idempotency_key, balance_after) \
                 VALUES ($1, 1, 'daily_free', 'checkin', $2, $3)",
            )
            .bind(auth.id)
            .bind(&df_idem)
            .bind(tb + 1)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE gacha_ticket_balance SET balance = $2 WHERE user_id = $1",
            )
            .bind(auth.id)
            .bind(tb + 1)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            df_granted = true;
        }
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }

    Ok(ok(serde_json::json!({
        "streak": reward.streak, "reward": reward.total,
        "base": reward.base, "streak_bonus": reward.streak_bonus,
        "gacha_ticket": df_granted,
    })))
}

#[get("/attendance")]
async fn checkin_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        // recent 列表含补签行（日历要展示）；streak 基线另查最后一条记录（补签也参与
        // 续签，§9.3 修订），见下方 last_live。
                "SELECT date, streak, \
         reward FROM attendance WHERE user_id = $1 AND date >= current_date - interval '30 days' ORDER BY date",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive(); // 站点时区 UTC+8
    let checked_today = rows.iter().any(|(d, _, _)| *d == today);
    // streak 基线取「最后一条记录」——补签行也参与续签（§9.3 修订，与签到主流程
    // 同口径）。补签行 streak 已由历史重算填成真实连续值，不再作废。
    // P-471：断一天起展示必须归 0——旧写法把库里那格的旧计数原样端出去，
    // 会员首页看到「连签 12 天」、点下去回的却是 streak=1。
    let last_live: Option<(chrono::NaiveDate, i32)> = sqlx::query_as(
        "SELECT date, streak FROM attendance WHERE user_id = $1          ORDER BY date DESC LIMIT 1",
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
                                     WHERE r.idempotency_key = concat('resub:', o.id))) \
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
    Ok(ok(serde_json::json!({
        "checked_today": checked_today, "streak": current_streak,
        "recent": rows, "makeup_cards": makeup_cards,
    })))
}
