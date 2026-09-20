//! 补签卡使用、等级规则与进度、教材愿望单（0020/0074）。
//! 从 gaps_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
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
    // 是否持有补签卡（未使用的订单）。
    // kind 兼容：商店种子为 makeup_card，本流程历史引用 resub_card——两种都认（0066 修复）。
    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if owned < 1 {
        return Err(DomainError::Validation(
            "没有可用补签卡：商店购买或管理发放后可在此使用".into(),
        ));
    }
    // 已签过则拒绝
    let signed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance WHERE user_id = $1 AND date = $2)",
    )
    .bind(auth.id)
    .bind(date)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if signed {
        return Err(DomainError::Validation("该日已有签到记录".into()));
    }
    // 消耗一张卡 + 补签记录 + 出勤记录（事务）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let order_id: i64 = sqlx::query_scalar(
        "SELECT o.id FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id)) \
         ORDER BY o.id LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO resub_uses (user_id, target_date, idempotency_key) VALUES ($1, $2, $3)",
    )
    .bind(auth.id)
    .bind(date)
    .bind(format!("resub:{order_id}"))
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
    // 返回剩余持有数（前端按钮展示用）
    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
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

#[derive(sqlx::FromRow, serde::Serialize)]
struct WishRow {
    id: i64,
    keyword: String,
    category_id: Option<i32>,
    grade_id: Option<i32>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/wishlist")]
pub async fn wishlist_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<WishRow> = sqlx::query_as(
        "SELECT id, keyword, category_id, grade_id, created_at FROM wishlist          WHERE user_id = $1 ORDER BY id DESC LIMIT 100",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WishAddReq {
    keyword: String,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    grade_id: Option<i32>,
}

#[post("/wishlist")]
pub async fn wishlist_add(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishAddReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let kw = body.keyword.trim();
    if kw.is_empty() || kw.len() > 100 {
        return Err(DomainError::Validation("关键词长度 1-100".into()));
    }
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM wishlist WHERE user_id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    if count >= 20 {
        return Err(DomainError::Validation(
            "愿望单上限 20 条，请先删除旧的".into(),
        ));
    }
    sqlx::query(
        "INSERT INTO wishlist (user_id, keyword, category_id, grade_id) VALUES ($1, $2, $3, $4)          ON CONFLICT (user_id, keyword) DO UPDATE SET category_id = EXCLUDED.category_id, grade_id = EXCLUDED.grade_id",
    )
    .bind(auth.id)
    .bind(kw)
    .bind(body.category_id)
    .bind(body.grade_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "keyword": kw })))
}

#[derive(Deserialize)]
struct WishDelReq {
    id: i64,
}

#[post("/wishlist/remove")]
pub async fn wishlist_remove(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WishDelReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query("DELETE FROM wishlist WHERE id = $1 AND user_id = $2")
        .bind(body.id)
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.id));
    }
    Ok(ok(serde_json::json!({ "removed": body.id })))
}
