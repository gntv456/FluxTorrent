//! Prometheus 指标。
//! 从 tracker main.rs 按域拆出。

use super::helpers::TrackerState;
use actix_web::{get, web, HttpResponse};
use std::sync::atomic::Ordering;

/// Prometheus 指标（/metrics，需 ANN_METRICS_TOKEN）

/// 本地滑动窗口限流（Redis 故障降级用）：key → (计数, 窗口起点)

/// Prometheus 指标端点：ANN_METRICS_TOKEN 未设置时返回 404（不暴露）；
/// 抓取端需带 X-Metrics-Token 头。监控与告警基线见 生产部署指南「监控与告警」节。
#[get("/metrics")]
pub(crate) async fn metrics(
    state: web::Data<TrackerState>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let tok = std::env::var("ANN_METRICS_TOKEN").unwrap_or_default();
    if tok.is_empty()
        || req
            .headers()
            .get("x-metrics-token")
            .and_then(|v| v.to_str().ok())
            != Some(tok.as_str())
    {
        return HttpResponse::NotFound().finish();
    }
    let m = &state.metrics;
    // 全量 GC 挂在抓取周期（P0-3：平时各桶惰性清理，这里兜底回收空桶与超时 peer）
    state.peers.gc_all();
    let (passkey_cache, ip_bans, agent_rules, swarms) = {
        let g = state.guard_read();
        (
            g.passkeys.len(),
            g.ip_bans.len(),
            g.agent_rules.as_ref().map_or(0, |r| r.len()),
            state.peers.swarms(),
        )
    };
    let body = format!(
        concat!(
            "# TYPE flux_tracker_announce_total counter\n",
            "flux_tracker_announce_total {}\n",
            "# TYPE flux_tracker_announce_ip_banned_total counter\n",
            "flux_tracker_announce_ip_banned_total {}\n",
            "# TYPE flux_tracker_announce_limited_ip_total counter\n",
            "flux_tracker_announce_limited_ip_total {}\n",
            "# TYPE flux_tracker_announce_limited_user_total counter\n",
            "flux_tracker_announce_limited_user_total {}\n",
            "# TYPE flux_tracker_announce_auth_fail_total counter\n",
            "flux_tracker_announce_auth_fail_total {}\n",
            "# TYPE flux_tracker_announce_agent_blocked_total counter\n",
            "flux_tracker_announce_agent_blocked_total {}\n",
            "# TYPE flux_tracker_announce_global_shed_total counter\n",
            "flux_tracker_announce_global_shed_total {}\n",
            "# TYPE flux_tracker_scrape_total counter\n",
            "flux_tracker_scrape_total {}\n",
            "# TYPE flux_tracker_redis_fallback_total counter\n",
            "flux_tracker_redis_fallback_total {}\n",
            "# TYPE flux_tracker_peers_active gauge\n",
            "flux_tracker_peers_active {}\n",
            "# TYPE flux_tracker_swarms_active gauge\n",
            "flux_tracker_swarms_active {}\n",
            "# TYPE flux_tracker_guard_cache gauge\n",
            "flux_tracker_guard_cache{{kind=\"passkey\"}} {}\n",
            "flux_tracker_guard_cache{{kind=\"ip_ban\"}} {}\n",
            "flux_tracker_guard_cache{{kind=\"agent_rule\"}} {}\n",
        ),
        m.announce_total.load(Ordering::Relaxed),
        m.announce_ip_banned.load(Ordering::Relaxed),
        m.announce_limited_ip.load(Ordering::Relaxed),
        m.announce_limited_user.load(Ordering::Relaxed),
        m.announce_auth_fail.load(Ordering::Relaxed),
        m.announce_agent_blocked.load(Ordering::Relaxed),
        m.announce_global_shed.load(Ordering::Relaxed),
        m.scrape_total.load(Ordering::Relaxed),
        m.redis_fallback.load(Ordering::Relaxed),
        state.peers.len(),
        swarms,
        passkey_cache,
        ip_bans,
        agent_rules,
    );
    HttpResponse::Ok()
        .content_type("text/plain; version=0.0.4")
        .body(body)
}
