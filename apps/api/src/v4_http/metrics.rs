//! G3：api 侧 Prometheus /metrics 与请求计数中间件（从 v4_http.rs 按域拆出）。
//! token 门禁同 tracker 口径。

use actix_web::body::MessageBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{get, web, HttpRequest, HttpResponse};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::state::AppState;

/// 全进程请求计数（path 维度聚合到方法级，防标签爆炸）
pub static METRIC_REQUESTS: AtomicU64 = AtomicU64::new(0);
pub static METRIC_REQUESTS_5XX: AtomicU64 = AtomicU64::new(0);
/// RED 之 Duration（U5 §12.6）：毫秒桶直方图（50/100/250/500/1000/2500/5000/10000+）
static LATENCY_BUCKETS_MS: [u64; 8] =
    [50, 100, 250, 500, 1000, 2500, 5000, 10000];
static METRIC_LATENCY: [AtomicU64; 8] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static METRIC_LATENCY_SUM_MS: AtomicU64 = AtomicU64::new(0);

fn record_latency(ms: u64) {
    METRIC_LATENCY_SUM_MS.fetch_add(ms, Ordering::Relaxed);
    for (i, b) in LATENCY_BUCKETS_MS.iter().enumerate() {
        if ms <= *b {
            METRIC_LATENCY[i].fetch_add(1, Ordering::Relaxed);
            return;
        }
    }
}

/// 请求计数中间件（from_fn 包装；计数是尽力而为，不影响请求路径）
pub async fn metrics_mw(
    req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> actix_web::Result<ServiceResponse<impl MessageBody>> {
    METRIC_REQUESTS.fetch_add(1, Ordering::Relaxed);
    let started = std::time::Instant::now();
    let res = next.call(req).await?;
    if res.status().as_u16() >= 500 {
        METRIC_REQUESTS_5XX.fetch_add(1, Ordering::Relaxed);
    }
    record_latency(started.elapsed().as_millis() as u64);
    Ok(res)
}

#[get("/metrics")]
pub(super) async fn api_metrics(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> HttpResponse {
    // 与 tracker /metrics 同款门禁：未配 token 时 404 不暴露。
    // 统一读 ANN_METRICS_TOKEN（compose/.env.example 一直按此名注入两处），
    // 此前误读 API_METRICS_TOKEN 导致 compose 配了 token 后 api /metrics 仍 404。
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
    // 池状态是 api 侧最有价值的 gauge（容量规划的底线数据）
    let pool = &state.repo.db;
    let pool_size = pool.size();
    let pool_idle = pool.num_idle();
    // U1 §12.6：announce 死信队列深度（billing 健康的第一信号；llen O(1)）。
    // 计费消费游标 lag 在 worker 侧无 API 可读，DLQ 深度 + 下方 stream 长度已覆盖「积压」口径。
    use redis::AsyncCommands;
    let mut conn = state.redis.clone();
    let dlq_depth: i64 = conn.llen("flux:announce:dlq").await.unwrap_or(0);
    let stream_len: i64 = conn.xlen("flux:announce").await.unwrap_or(0);
    // RED 直方图（累计桶 → Prometheus cumulative 口径）
    let mut hist = String::new();
    for (i, b) in LATENCY_BUCKETS_MS.iter().enumerate() {
        hist.push_str(&format!(
            "flux_api_request_duration_bucket{{le=\"{}\"}} {}\n",
            b,
            METRIC_LATENCY[i].load(Ordering::Relaxed)
        ));
    }
    hist.push_str(&format!(
        "flux_api_request_duration_bucket{{le=\"+Inf\"}} {}\n",
        METRIC_REQUESTS.load(Ordering::Relaxed)
    ));
    hist.push_str(&format!(
        "# HELP flux_api_request_duration_ms_sum Total request duration in ms.\n# TYPE flux_api_request_duration_ms_sum counter\nflux_api_request_duration_ms_sum {}\n",
        METRIC_LATENCY_SUM_MS.load(Ordering::Relaxed)
    ));
    let body = format!(
        concat!(
            "# HELP flux_api_requests_total Total requests since start.\n",
            "# TYPE flux_api_requests_total counter\n",
            "flux_api_requests_total {}\n",
            "# HELP flux_api_requests_5xx_total Responses with status >= 500.\n",
            "# TYPE flux_api_requests_5xx_total counter\n",
            "flux_api_requests_5xx_total {}\n",
            "# HELP flux_api_pool_connections Current PG pool connections.\n",
            "# TYPE flux_api_pool_connections gauge\n",
            "flux_api_pool_connections {}\n",
            "# HELP flux_api_pool_idle Idle PG pool connections.\n",
            "# TYPE flux_api_pool_idle gauge\n",
            "flux_api_pool_idle {}\n",
            "# HELP flux_api_announce_dlq_depth Dead-letter queue depth (corrupt announce events).\n",
            "# TYPE flux_api_announce_dlq_depth gauge\n",
            "flux_api_announce_dlq_depth {}\n",
            "# HELP flux_api_announce_stream_len announce Redis Stream total entries (grows until trim).\n",
            "# TYPE flux_api_announce_stream_len gauge\n",
            "flux_api_announce_stream_len {}\n",
            "# TYPE flux_api_request_duration_bucket counter\n",
        ),
        METRIC_REQUESTS.load(Ordering::Relaxed),
        METRIC_REQUESTS_5XX.load(Ordering::Relaxed),
        pool_size,
        pool_idle,
        dlq_depth,
        stream_len,
    );
    let body = body + &hist;
    HttpResponse::Ok().content_type("text/plain").body(body)
}
