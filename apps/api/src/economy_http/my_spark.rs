//! 我的魔力概览端点（M11）。
//! 自 voucher_use.rs 按域拆出（2026-11-17，300 行门禁）：券核销留在
//! voucher_use.rs，本文件只管 /me/spark 概览聚合。

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, Responder};

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
    // 0488：官种做种数与体积（只读展示，不参与结算）。
    let (official_seeding_count, official_seeding_size) =
        super::seeding_est::official_seeding(&state.repo.db, auth.id).await;
    // 与 worker 同口径的僵尸阈值：max(2h, 2×announce_interval)
    // （详见 worker 的 stale_peer_threshold_secs）。
    // 口径抽到 seeding_public::stale_threshold_secs：开放 API 的
    // /open/me/spark（插件用户卡的时魔）走同一份实现，两边不会漂移。
    let stale_secs =
        super::seeding_public::stale_threshold_secs(&state.repo).await;
    // 收益构成：按规则名分组（种子档位 + 做种时长档位两类），与 worker 结算同源。
    // 0133 后新增「做种时长」维度 —— 玩家能看到"我为什么拿这么多"，这是相对 NP 的体验优势。
    let my_rules: Vec<(String, i64)> = sqlx::query_as(
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
              -- 与结算同口径（0488 核对）：构成表只统计真正计收益的种子，
              -- 否则「构成」里的颗数会多于页面上的计收益颗数。
              AND s.last_port > 0
              AND NOT COALESCE(s.connectable = 0, false)
              AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
        ) x GROUP BY rule ORDER BY count(*) DESC
        "#,
    )
    .bind(auth.id)
    .bind(stale_secs)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    // 真实预估：语句与 worker 结算逐条镜像（见 seeding_est.rs 的 EST_SQL）。
    let hourly_estimate = super::seeding_est::hourly_estimate(
        &state.repo.db,
        auth.id,
        stale_secs,
    )
    .await;
    // 预计时积分（2026-11-17）：做种积分 = 做种小时数，每小时约 +活跃颗数。
    // 闸门比收益宽（无端口/回连/停权），故与 hourly_estimate 数值可不同。
    let hourly_points_estimate = super::seeding_est::hourly_points_estimate(
        &state.repo.db,
        auth.id,
        stale_secs,
    )
    .await;
    // 「做种 6 颗，只有 4 颗计收益」必须说清是哪 2 颗、为什么 —— 直接扣掉
    // 不给原因，用户只会以为站点算错。
    let split: super::seeding_est::EarningSplit =
        super::seeding_est::earning_split(&state.repo.db, auth.id, stale_secs)
            .await;
    // 明细清单（2026-11-18 工单诉求）：「不可达 34 颗」要能点开看是哪 34 颗。
    // SQL 在 repo 层 seed_exclusion（FRAME §2 棘轮），分类 CASE 与
    // seeding_est::SPLIT_SQL 逐字一致。
    let excluded_list: Vec<serde_json::Value> = state
        .repo
        .seed_exclusion_detail(auth.id, stale_secs)
        .await
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "torrent_id": r.torrent_id, "name": r.name, "cat": r.cat,
            })
        })
        .collect();
    // 0417 F6 / 0488：系数表构造按域拆到 spark_tiers.rs（300 行门禁）。
    // 参数仍从 seeding_params() + seeding_official_mult() 取真值，与结算同源。
    // 0525：传入用户 id，系数表带上该用户的等级加成率。
    let tiers = super::spark_tiers::build_tiers(&state.repo.db, auth.id).await;
    // 0525 流量积分待结算块：与 worker/traffic_reward.rs 同源口径——
    // 基线差（GREATEST 钳 0）按 GB 换算，floor 后即下次结算预计入账。
    // delta 重算式较长，收成子查询 pend 一处求值，外层只引用。
    let traffic: Option<(f64, f64, f64, f64, i64)> = sqlx::query_as(
        "WITH rates AS ( \
           SELECT COALESCE((SELECT value::float8 FROM site_settings \
                     WHERE name='seeding_traffic_up_rate'), 0.5) AS up, \
                  COALESCE((SELECT value::float8 FROM site_settings \
                     WHERE name='seeding_traffic_down_rate'), 0.1) AS down), \
         pend AS ( \
           SELECT GREATEST(u.uploaded - COALESCE(t.last_uploaded, \
                                        u.uploaded), 0)::float8 AS up_b, \
                  GREATEST(u.downloaded - COALESCE(t.last_downloaded, \
                                          u.downloaded), 0)::float8 AS down_b \
           FROM users u \
           LEFT JOIN traffic_settle_state t ON t.user_id = u.id \
           WHERE u.id = $1), \
         scored AS ( \
           SELECT p.up_b, p.down_b, \
                  FLOOR(p.up_b / 1073741824.0 * rates.up \
                      + p.down_b / 1073741824.0 * rates.down)::bigint AS est \
           FROM pend p CROSS JOIN rates) \
         SELECT rates.up, rates.down, s.up_b, s.down_b, s.est \
         FROM scored s CROSS JOIN rates",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    let traffic_json = match traffic {
        Some((up, down, up_bytes, down_bytes, est)) => serde_json::json!({
            "up_rate_per_gb": up,
            "down_rate_per_gb": down,
            "pending_up_gb": (up_bytes / 1073741824.0 * 1000.0).round()
                / 1000.0,
            "pending_down_gb": (down_bytes / 1073741824.0 * 1000.0).round()
                / 1000.0,
            "pending_points_estimate": est,
        }),
        None => serde_json::json!({
            "up_rate_per_gb": 0.5,
            "down_rate_per_gb": 0.1,
            "pending_up_gb": 0.0,
            "pending_down_gb": 0.0,
            "pending_points_estimate": 0,
        }),
    };
    Ok(ok(serde_json::json!({
        "balance": balance,
        "seeding_count": seeding_count,
        "earning_seeding_count": split.earning,
        "seeding_excluded": {
            "port_closed": split.port_closed,
            "unreachable": split.unreachable,
            "stale": split.stale,
        },
        "excluded_detail": excluded_list,
        "payout_suspended": split.suspended,
        "official_seeding_count": official_seeding_count,
        "official_seeding_size": official_seeding_size,
        "hourly_estimate": hourly_estimate,
        "hourly_points_estimate": hourly_points_estimate,
        // 0525 流量积分：费率 + 待结算量与预计积分（基线差口径）
        "traffic": traffic_json,
        // 兼容旧消费点（my-spark pill 渲染）：保持 [规则名, 颗数] 数组形态
        "reward_rules": my_rules,
        "tiers": tiers,
        "formula_note": super::spark_tiers::FORMULA_NOTE,
    })))
}
