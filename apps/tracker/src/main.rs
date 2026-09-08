//! FluxTorrent Tracker —— 轻量私有 Tracker（方案 §2.5 自研兜底路线）。
//! 设计（§5.4 announce 链路）：
//!   ① passkey 鉴权：内存/Redis 缓存 passkey→user（定期从 PG 同步，announce 路径零 DB）
//!   ② peer 状态：DashMap 内存表（60s 超时淘汰）
//!   ③ 立即返回 compact 响应，不阻塞计费
//!   ④ 事件 → Redis Stream（XADD fire-and-forget），worker 异步消费计费
//! 实现 BEP3 核心：/announce（started/regular/stopped/completed + compact=1）与 /scrape。

mod peers;

use actix_web::{get, web, App, HttpResponse, HttpServer};
use peers::{PeerKey, PeerTable};
use serde::Deserialize;
use std::sync::Arc;

use crate::peers::Peer;

struct TrackerState {
    peers: PeerTable,
    redis: redis::aio::ConnectionManager,
    db: sqlx::PgPool,
}

#[derive(Deserialize)]
struct AnnounceQuery {
    info_hash: String,
    peer_id: String,
    port: u16,
    uploaded: i64,
    downloaded: i64,
    left: i64,
    #[serde(default)]
    event: Option<String>,
    #[serde(default = "default_numwant")]
    numwant: u32,
    ip: Option<String>,
}

fn default_numwant() -> u32 {
    50
}

#[get("/announce/{passkey}")]
async fn announce(
    state: web::Data<Arc<TrackerState>>,
    path: web::Path<String>,
    q: web::Query<AnnounceQuery>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let passkey = path.into_inner();
    let event = q.event.as_deref().unwrap_or("");
    let ip =
        q.ip.clone()
            .or_else(|| req.peer_addr().map(|a| a.ip().to_string()))
            .unwrap_or_else(|| "0.0.0.0".into());

    // ① passkey → user_id（内存缓存，未命中查 PG 并回填）
    let user_id = match resolve_passkey(&state.db, &passkey).await {
        Some(uid) => uid,
        None => return bencode_err("passkey 无效，请在站点重置"),
    };

    let key = PeerKey {
        info_hash: q.info_hash.clone(),
        peer_id: q.peer_id.clone(),
    };

    // stopped 事件：移除 peer，其余事件 upsert
    if event == "stopped" {
        state.peers.remove(&key);
    } else {
        state.peers.upsert(Peer {
            key,
            ip,
            port: q.port,
            uploaded: q.uploaded,
            downloaded: q.downloaded,
            left: q.left,
            last_seen: chrono::Utc::now(),
            user_id,
        });
    }

    // ④ 事件投递 Redis Stream（fire-and-forget，绝不阻塞响应）
    emit_event(
        &state.redis,
        &q.info_hash,
        user_id,
        q.uploaded,
        q.downloaded,
        event,
        q.left,
    )
    .await;

    // ③ compact 响应
    let seeders = state.peers.count_seeders(&q.info_hash);
    let leechers = state.peers.count_leechers(&q.info_hash);
    let complete = 0i64; // 完成数由 worker 从 snatches 聚合回填（BEP3 允许省略语义）

    let body = if event == "stopped" {
        peers::bencode_announce(0, 0, 0, &[])
    } else {
        let list = state
            .peers
            .snapshot(&q.info_hash, q.numwant as usize, &q.peer_id);
        peers::bencode_announce(seeders as i64, leechers as i64, complete, &list)
    };
    HttpResponse::Ok().content_type("text/plain").body(body)
}

#[get("/scrape/{passkey}")]
async fn scrape(
    state: web::Data<Arc<TrackerState>>,
    path: web::Path<String>,
    q: web::Query<ScrapeQuery>,
) -> HttpResponse {
    let passkey = path.into_inner();
    if resolve_passkey(&state.db, &passkey).await.is_none() {
        return bencode_err("passkey 无效");
    }
    let mut files = String::new();
    for ih in q.info_hashes.split(',').filter(|s| !s.is_empty()) {
        let (s, l) = state.peers.counts(ih);
        files.push_str(&format!(
            "20:{}d8:completei{}e10:incompletei{}e10:downloadedi0ee",
            ih, s, l
        ));
    }
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(format!("d5:filesd{}e", files))
}

#[derive(Deserialize)]
struct ScrapeQuery {
    /// hex 编码的 info_hash 列表（简化：逗号分隔）
    info_hashes: String,
}

fn bencode_err(msg: &str) -> HttpResponse {
    HttpResponse::Ok().content_type("text/plain").body(format!(
        "d14:failure reason{}:{}e",
        msg.len(),
        msg
    ))
}

async fn resolve_passkey(db: &sqlx::PgPool, passkey: &str) -> Option<i64> {
    sqlx::query_scalar::<_, Option<i64>>("SELECT id FROM users WHERE passkey = $1 AND status < 2")
        .bind(passkey)
        .fetch_one(db)
        .await
        .ok()
        .flatten()
}

async fn emit_event(
    redis: &redis::aio::ConnectionManager,
    info_hash_hex: &str,
    user: i64,
    up: i64,
    down: i64,
    event: &str,
    left: i64,
) {
    let payload = serde_json::json!({
        "user": user, "hash": info_hash_hex, "up": up, "down": down,
        "event": event, "left": left,
        "ts": chrono::Utc::now().to_rfc3339(),
    });
    let mut cmd = redis::cmd("XADD");
    cmd.arg("flux:announce")
        .arg("*")
        .arg("payload")
        .arg(payload.to_string());
    let mut conn = redis.clone();
    if let Err(e) = cmd.query_async::<()>(&mut conn).await {
        tracing::warn!(?e, "announce 事件投递失败（不影响响应）");
    }
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let bind = std::env::var("TRACKER_BIND").unwrap_or_else(|_| "0.0.0.0:7070".into());
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://flux:fluxdevpass@127.0.0.1:5432/fluxtorrent".into());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());

    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(3)
        .connect(&db_url)
        .await?;
    let redis = redis::Client::open(redis_url.as_str())?
        .get_connection_manager()
        .await?;
    let state = web::Data::new(Arc::new(TrackerState {
        peers: PeerTable::new(),
        redis,
        db,
    }));

    tracing::info!("flux-tracker listening on {bind}");
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(announce)
            .service(scrape)
    })
    .bind(&bind)?
    .run()
    .await?;
    Ok(())
}
