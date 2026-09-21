//! 免费券使用（0073）。
//! 从 economy_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

/// 用券：把一张未用未过期的券绑定到种子（CAS 防并发双用）。
/// 生效在 worker 计费侧：该种该用户的下载增量按 0 计（free）/上下行均 0 计（neutral），
/// 当累计下载超过种子大小 4% 时核销（Gazelle slop 口径——防买了券只下 1% 就转移给别人用）。
#[post("/me/vouchers/use")]
async fn voucher_use(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<VoucherUseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 绑定即生效但不置 used_at：worker 计费侧以「used_torrent_id 已绑定 + used_at IS NULL」
    // 判定生效中的券，下载量过阈值后由核销语句置 used_at。旧版绑定时就写 used_at，
    // 导致券永远不被计费侧匹配（用户花钱买的权益确定性为 0，真实资损）。
    let n = sqlx::query(
        "UPDATE user_vouchers SET used_torrent_id = $3 \
         WHERE id = $1 AND user_id = $2 AND used_torrent_id IS NULL AND used_at IS NULL AND expires_at > now()",
    )
    .bind(body.voucher_id)
    .bind(auth.id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("券不存在、已使用或已过期".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "voucher.use", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "voucher_id": body.voucher_id,
        "torrent_id": body.torrent_id,
        "note": "已对该种子生效；下载量超过种子大小 4% 后自动核销"
    })))
}

// ============ 我的火花（M11） ============

#[get("/me/spark")]
async fn my_spark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    // 收益因子说明（设计稿：1.03x/5x/0.1x 可点击展开）—— 由做种状态推导
    let seeding_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM snatches WHERE user_id = $1 AND seeding",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 与 worker 同口径的僵尸阈值：max(2h, 2×announce_interval)（详见 worker 的 stale_peer_threshold_secs）
    let announce: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    let stale_secs = (announce * 2).max(7200);
    // 收益构成：按规则名分组（种子档位 + 做种时长档位两类），与 worker 结算同源。
    // 0133 后新增「做种时长」维度 —— 玩家能看到"我为什么拿这么多"，这是相对 NP 的体验优势。
    let rules: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT rule, count(*) FROM (
            SELECT CASE
                     WHEN t.seeders <= 1 AND t.times_completed >= 3 THEN '濒危保种'
                     WHEN now() - t.created_at > interval '365 days' THEN '高龄种'
                     WHEN now() - t.created_at > interval '180 days' THEN '老种'
                     WHEN t.size >= 107374182400 THEN '大体积种'
                     WHEN t.size >= 26843545600 THEN '中体积种'
                     ELSE '日常种'
                   END AS rule
            FROM snatches s JOIN torrents t ON t.id = s.torrent_id
            WHERE s.user_id = $1 AND s.seeding
              AND NOT (s.connectable = 0 AND s.uploaded = 0)
              AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
            UNION ALL
            SELECT CASE
                     WHEN s.seeded_seconds >= 31536000 THEN '传奇做种者（≥1年）'
                     WHEN s.seeded_seconds >= 15552000 THEN '资深做种者（6-12月）'
                     WHEN s.seeded_seconds >= 7776000 THEN '稳定做种者（3-6月）'
                     WHEN s.seeded_seconds >= 2592000 THEN '新晋做种者（1-3月）'
                     ELSE '新做种（<1月）'
                   END AS rule
            FROM snatches s
            WHERE s.user_id = $1 AND s.seeding
              AND NOT (s.connectable = 0 AND s.uploaded = 0)
              AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
        ) x GROUP BY rule ORDER BY count(*) DESC
        "#,
    )
    .bind(auth.id)
    .bind(stale_secs)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    // 真实预估（与 worker 结算同一套 DB 函数 + 同一套过滤，迁移 0133）：
    // 不再是 "10 + 颗数×2" 的粗估。
    let hourly_estimate: i64 = sqlx::query_scalar(
        r#"
        WITH p AS MATERIALIZED (SELECT * FROM seeding_params()),
        s AS (
            SELECT sum(seeding_torrent_bonus(
                       t.size, t.seeders,
                       (EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0)::double precision,
                       t.times_completed,
                       (GREATEST(sn.seeded_seconds, 0) / 3600.0)::double precision,
                       p.vol_base, p.rarity_k, p.rarity_exp, p.scale)) AS bonus_raw,
                   bool_or(u.donor) AS donor
            FROM snatches sn
            JOIN torrents t ON t.id = sn.torrent_id
            JOIN users u ON u.id = sn.user_id
            CROSS JOIN p
            WHERE sn.user_id = $1 AND sn.seeding
              AND NOT (sn.connectable = 0 AND sn.uploaded = 0)
              AND sn.last_seen_at > now() - ($2::bigint * interval '1 second')
        )
        SELECT seeding_hourly(COALESCE(bonus_raw, 0), p.base, p.cap, p.curve_k, p.donor_mult,
                              COALESCE(donor, FALSE))
        FROM s CROSS JOIN p
        "#,
    )
    .bind(auth.id)
    .bind(stale_secs)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    Ok(ok(serde_json::json!({
        "balance": balance,
        "seeding_count": seeding_count,
        "hourly_estimate": hourly_estimate,
        "reward_rules": rules,
        "formula_note": "每小时 = 底薪 + (2/π)·封顶·atan(Σ加成 × 曲线系数)，捐赠者整笔翻倍。每颗种子加成 =（种子档位分 + 做种时长档位分）× 体积因子 × 稀有度加成 × 标定系数；种子档位：濒危保种 2.0 / 高龄种 1.5 / 老种 1.0 / 大体积 0.75 / 中体积 0.5 / 日常 0.25；做种时长档位：≥1年 +2.0 / 6-12月 +1.0 / 3-6月 +0.75 / 1-3月 +0.5；体积按对数饱和（≥50GB 记满分，1GB 约 0.18）；稀有度独苗 ×1.6、人多趋近 ×1.0。",
    })))
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct LedgerRow {
    amount: i64,
    kind: String,
    balance_after: Option<i64>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct LedgerQuery {
    limit: Option<i64>,
}

#[get("/me/spark/ledger")]
async fn my_ledger(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LedgerQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, LedgerRow>(
        "SELECT amount, kind, balance_after, created_at FROM spark_ledger \
         WHERE user_id = $1 ORDER BY id DESC LIMIT $2",
    )
    .bind(auth.id)
    .bind(q.limit.unwrap_or(20).clamp(1, 50))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
pub(super) struct VoucherUseReq {
    pub(super) voucher_id: i64,
    pub(super) torrent_id: i64,
}
