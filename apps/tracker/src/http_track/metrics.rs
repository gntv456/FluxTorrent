//! Prometheus 指标。
//! 从 tracker main.rs 按域拆出。

use super::helpers::TrackerState;
use actix_web::{get, web, HttpResponse};
use std::sync::atomic::Ordering;

/// Prometheus 指标（/metrics，需 ANN_METRICS_TOKEN）

/// 本地滑动窗口限流（Redis 故障降级用）：key → (计数, 窗口起点)

/// 指标端点鉴权（ZT81 2026-10-02）：同时接受 `x-metrics-token` 与标准
/// `Authorization: Bearer <token>`。原实现只认前者，而随仓库附带的
/// `docker/monitoring/prometheus.yml` 用的是标准 `authorization: Bearer`
/// → 即便按文档配好 ANN_METRICS_TOKEN，Prometheus 抓取仍全 404，监控与
/// 告警全部不触发（「有监控栈」是假象）。
pub(crate) fn metrics_authorized(req: &actix_web::HttpRequest) -> bool {
    let tok = std::env::var("ANN_METRICS_TOKEN").unwrap_or_default();
    if tok.is_empty() {
        return false;
    }
    let hdr = |n: &str| {
        req.headers()
            .get(n)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    };
    let bearer = hdr("authorization")
        .strip_prefix("Bearer ")
        .unwrap_or_default()
        .trim();
    ct_eq(hdr("x-metrics-token"), tok.clone())
        || ct_eq(bearer, tok)
}

/// 定长时间比较：`/metrics` 挂在公网 :7070 上，`==` 的短路会把 token
/// 变成可逐字节测的侧信道（无速率限制的那条路径）。
fn ct_eq(a: &str, b: String) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

/// Prometheus 指标端点：ANN_METRICS_TOKEN 未设置时返回 404（不暴露）；
/// 抓取端带 X-Metrics-Token 或 Authorization: Bearer 均可。监控与告警基线见
/// 生产部署指南「监控与告警」节。
#[get("/metrics")]
pub(crate) async fn metrics(
    state: web::Data<TrackerState>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    if !metrics_authorized(&req) {
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
            "# TYPE flux_tracker_announce_torrent_unknown_total counter\n",
            "flux_tracker_announce_torrent_unknown_total {}\n",
            "# TYPE flux_tracker_announce_global_shed_total counter\n",
            "flux_tracker_announce_global_shed_total {}\n",
            "# TYPE flux_tracker_scrape_total counter\n",
            "flux_tracker_scrape_total {}\n",
            "# TYPE flux_tracker_redis_fallback_total counter\n",
            "flux_tracker_redis_fallback_total {}\n",
            "# TYPE flux_tracker_ip_inject_rejected_total counter\n",
            "flux_tracker_ip_inject_rejected_total {}\n",
            "# TYPE flux_tracker_passkey_query_failed_total counter\n",
            "flux_tracker_passkey_query_failed_total {}\n",
            "# TYPE flux_tracker_announce_fake_left_total counter\n",
            "flux_tracker_announce_fake_left_total {}\n",
            "# TYPE flux_tracker_announce_peer_taken_total counter\n",
            "flux_tracker_announce_peer_taken_total {}\n",
            "# TYPE flux_tracker_announce_evt_merged_total counter\n",
            "flux_tracker_announce_evt_merged_total {}\n",
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
        m.announce_torrent_unknown.load(Ordering::Relaxed),
        m.announce_global_shed.load(Ordering::Relaxed),
        m.scrape_total.load(Ordering::Relaxed),
        m.redis_fallback.load(Ordering::Relaxed),
        super::ip_trust::ip_inject_rejected(),
        m.passkey_query_failed.load(Ordering::Relaxed),
        m.announce_fake_left.load(Ordering::Relaxed),
        m.announce_peer_taken.load(Ordering::Relaxed),
        m.announce_evt_merged.load(Ordering::Relaxed),
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
