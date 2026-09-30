//! 补签卡使用、等级规则与进度、教材愿望单（0020/0074）。
//! 从 gaps_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 补签卡使用 ============

#[derive(Deserialize)]
struct ResubReq {
    target_date: String, // YYYY-MM-DD
    #[allow(dead_code)]
    idempotency_key: String, // 幂等语义由 resub_uses.idempotency_key（订单维度）承载
}

/// 使用补签卡：前提是拥有该道具（shop_orders 中 kind='resub_card' 的有效订单）
#[post("/attendance/resub")]
pub async fn resub_use(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResubReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let date = chrono::NaiveDate::parse_from_str(&body.target_date, "%Y-%m-%d")
        .map_err(|_| DomainError::Validation("日期格式 YYYY-MM-DD".into()))?;
    // 只能补过去 7 天内
    let days_ago =
        ((chrono::Utc::now() + chrono::Duration::hours(8)).date_naive() - date)
            .num_days();
    if !(1..=7).contains(&days_ago) {
        return Err(DomainError::Validation("只能补过去 7 天内".into()));
    }
    // 是否持有补签卡。双源（0262 起）：商店未消耗订单 + 娱乐屋奖池发放的
    // 「补签卡」（发放账 − 消耗账，与背包同口径）—— 财神式「游戏产出 → 签到
    // 救急」闭环：刮刮乐抽到的卡就是能拿来补签的卡。
    // kind 兼容：商店种子为 makeup_card，本流程历史引用 resub_card——两种都认（0066 修复）。
    let owned: i64 = sqlx::query_scalar(
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
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if owned < 1 {
        return Err(DomainError::Validation(
            "没有可用补签卡：商店购买、奖池抽取或管理发放后可在此使用".into(),
        ));
    }
    // 已签过则拒绝
    let signed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance WHERE user_id = $1 AND \
         date = $2)",
    )
    .bind(auth.id)
    .bind(date)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if signed {
        return Err(DomainError::Validation("该日已有签到记录".into()));
    }
    // 消耗一张卡（先商店订单，没有再扣娱乐屋发放的）+ 补签记录 + 出勤记录（事务）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let shop_order: Option<i64> = sqlx::query_scalar(
        "SELECT o.id FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id)) \
         ORDER BY o.id LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let resub_idem = match shop_order {
        Some(order_id) => format!("resub:{order_id}"),
        None => {
            // 娱乐屋发放的卡：扣一件（use_kind='resub'），幂等键按「人 + 目标日」
            // 定死 —— 同一天重放不重复扣
            sqlx::query(
                "INSERT INTO arcade_item_uses \
                     (item_key, user_id, qty, use_kind, idem) \
                 VALUES ('resub_card', $1, 1, 'resub', $2)",
            )
            .bind(auth.id)
            .bind(format!(
                "resub-arc:{}:{}",
                auth.id,
                date.format("%Y%m%d")
            ))
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            format!("resub:arc:{}:{}", auth.id, date.format("%Y%m%d"))
        }
    };
    sqlx::query(
        "INSERT INTO resub_uses (user_id, target_date, \
         idempotency_key) VALUES ($1, $2, $3)",
    )
    .bind(auth.id)
    .bind(date)
    .bind(resub_idem)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 补签出勤行：streak 不续（补的是漏签日）、reward=0（火花在正常签到发放）
    sqlx::query(
        "INSERT INTO attendance (user_id, date, streak, reward, makeup) \
         VALUES ($1, $2, 0, 0, TRUE) ON CONFLICT (user_id, date) DO NOTHING",
    )
    .bind(auth.id)
    .bind(date)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 返回剩余持有数（前端按钮展示用；双源口径与上面一致）
    let left: i64 = sqlx::query_scalar(
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
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(
        serde_json::json!({ "resubbed": body.target_date, "cards_left": left }),
    ))
}

// ============ 等级规则与进度 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct ClassRuleRow {
    class_id: i32,
    name: String,
    min_uploaded: i64,
    min_download_count: i32,
    min_seed_hours: i32,
    min_account_age_days: i32,
}

#[get("/classes")]
pub async fn class_rules_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let rows: Vec<ClassRuleRow> = sqlx::query_as(
        "SELECT class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days \
         FROM class_rules ORDER BY class_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(sqlx::FromRow)]
struct ProgressRow {
    class_id: i32,
    uploaded: i64,
    download_count: i64,
    seed_hours: i64,
    account_age_days: i64,
}

#[get("/me/class-progress")]
pub async fn my_class_progress(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let p: ProgressRow = sqlx::query_as(
        "SELECT u.class_id, u.uploaded, \
                (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.completed_at IS NOT NULL) AS download_count, \
                (SELECT COALESCE(sum(s.seeded_seconds), 0) / 3600 FROM snatches s WHERE s.user_id = u.id) AS seed_hours, \
                EXTRACT(DAY FROM now() - u.created_at)::bigint AS account_age_days \
         FROM users u WHERE u.id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 计算当前进度满足到哪一级
    let rules: Vec<ClassRuleRow> = sqlx::query_as(
        "SELECT class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days \
         FROM class_rules ORDER BY class_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut qualified = 1i32;
    for r in &rules {
        if p.uploaded >= r.min_uploaded
            && p.download_count >= r.min_download_count as i64
            && p.seed_hours >= r.min_seed_hours as i64
            && p.account_age_days >= r.min_account_age_days as i64
        {
            qualified = r.class_id;
        }
    }
    Ok(ok(serde_json::json!({
        "current_class": p.class_id, "qualified_class": qualified,
        "uploaded": p.uploaded, "download_count": p.download_count,
        "seed_hours": p.seed_hours, "account_age_days": p.account_age_days,
    })))
}

// ============ 附件/截图与媒体信息（种子上传时提交） ============
// torrents.screenshots / media_info 列已由 0020 添加；上传表单扩展见 web 端。

// ============ 教材愿望单（0074，U3D WishList 教育化） ============
