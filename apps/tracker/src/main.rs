//! FluxTorrent Tracker —— 轻量私有 Tracker（方案 §2.5 自研兜底路线）。
//! 设计（§5.4 announce 链路）：
//!   ① passkey 鉴权：查 PG（后续可加内存缓存），拒绝无效 passkey
//!   ② peer 状态：DashMap 内存表（90s 超时淘汰）
//!   ③ 立即返回 compact 响应（二进制安全），不阻塞计费
//!   ④ 事件 → Redis Stream（XADD），worker 异步消费计费
//! BEP3 兼容要点：info_hash/peer_id 是任意字节的 percent-encoding ——
//! 绕过 serde Query 反序列化，直接解析原始 query 字节。

mod peers;

use actix_web::{get, web, App, HttpResponse, HttpServer};
use peers::{bencode_scrape, PeerKey, PeerTable};
use std::sync::Arc;

use crate::peers::{bencode_announce, hex, percent_decode, Peer};

struct TrackerState {
    peers: PeerTable,
    redis: redis::aio::ConnectionManager,
    db: sqlx::PgPool,
}

/// 从原始 query string 提取参数（字节层，percent-decode）
struct RawParams<'a> {
    raw: &'a str,
}

impl<'a> RawParams<'a> {
    fn new(raw: &'a str) -> Self {
        Self { raw }
    }
    /// 返回解码后的字节值
    fn get_bytes(&self, key: &str) -> Option<Vec<u8>> {
        for pair in self.raw.split('&') {
            let mut it = pair.splitn(2, '=');
            if it.next() == Some(key) {
                let v = it.next().unwrap_or("");
                return Some(percent_decode(v.as_bytes()));
            }
        }
        None
    }
    fn get_str(&self, key: &str) -> Option<String> {
        self.get_bytes(key)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    }
    fn get_i64(&self, key: &str, default: i64) -> i64 {
        self.get_str(key)
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    }
}

#[get("/announce/{passkey}")]
async fn announce(
    state: web::Data<Arc<TrackerState>>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let passkey = path.into_inner();
    let raw_query = req.query_string();

    let params = RawParams::new(raw_query);
    let Some(info_hash_raw) = params.get_bytes("info_hash") else {
        return bencode_err("缺少 info_hash");
    };
    if info_hash_raw.len() != 20 {
        return bencode_err("info_hash 必须为 20 字节");
    }
    let Some(peer_id_raw) = params.get_bytes("peer_id") else {
        return bencode_err("缺少 peer_id");
    };
    if peer_id_raw.is_empty() || peer_id_raw.len() > 20 {
        return bencode_err("peer_id 长度无效");
    }

    let info_hash_hex = hex(&info_hash_raw);
    let peer_id_hex = hex(&peer_id_raw);
    let port: u16 = params.get_i64("port", 0) as u16;
    let uploaded = params.get_i64("uploaded", 0);
    let downloaded = params.get_i64("downloaded", 0);
    let left = params.get_i64("left", 0);
    let numwant = params.get_i64("numwant", 50).clamp(1, 200) as usize;
    let event = params.get_str("event").unwrap_or_default();
    let event = event.as_str();
    let ip = params
        .get_str("ip")
        .or_else(|| req.peer_addr().map(|a| a.ip().to_string()))
        .unwrap_or_else(|| "0.0.0.0".into());

    // ① passkey → user_id
    let Some(user_id) = resolve_passkey(&state.db, &passkey).await else {
        return bencode_err("passkey 无效，请在站点重置");
    };

    let key = PeerKey {
        info_hash: info_hash_hex.clone(),
        peer_id: peer_id_hex.clone(),
    };

    // stopped：移除 peer；其余 upsert
    if event == "stopped" {
        state.peers.remove(&key);
    } else {
        state.peers.upsert(Peer {
            key: key.clone(),
            ip,
            port,
            uploaded,
            downloaded,
            left,
            last_seen: chrono::Utc::now(),
            user_id,
        });
    }

    // ④ 事件投递（fire-and-forget，失败仅告警）
    emit_event(
        &state.redis,
        &info_hash_hex,
        user_id,
        uploaded,
        downloaded,
        event,
        left,
    )
    .await;

    // ③ compact 二进制响应
    let body = if event == "stopped" {
        bencode_announce(0, 0, 0, &[])
    } else {
        let seeders = state.peers.count_seeders(&info_hash_hex);
        let leechers = state.peers.count_leechers(&info_hash_hex);
        let list = state.peers.snapshot(&info_hash_hex, numwant, &key.peer_id);
        bencode_announce(seeders as i64, leechers as i64, 0, &list)
    };
    HttpResponse::Ok().content_type("text/plain").body(body)
}

#[get("/scrape/{passkey}")]
async fn scrape(
    state: web::Data<Arc<TrackerState>>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let passkey = path.into_inner();
    if resolve_passkey(&state.db, &passkey).await.is_none() {
        return bencode_err("passkey 无效");
    }
    let params = RawParams::new(req.query_string());
    // BEP48：info_hash 可重复出现多次，每次为 20 字节 percent-encoding
    let mut files: Vec<(Vec<u8>, usize, usize)> = Vec::new();
    for pair in req.query_string().split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some("info_hash") {
            let raw = percent_decode(it.next().unwrap_or("").as_bytes());
            if raw.len() == 20 {
                let hexkey = hex(&raw);
                let (s, l) = state.peers.counts(&hexkey);
                files.push((raw, s, l));
            }
        }
    }
    let _ = params;
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(bencode_scrape(&files))
}

fn bencode_err(msg: &str) -> HttpResponse {
    // 中文按 UTF-8 字节数编码长度（bencode 长度前缀是字节数）
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
