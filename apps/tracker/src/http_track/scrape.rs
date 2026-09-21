//! scrape。
//! 从 tracker main.rs 按域拆出。

use super::emit::bencode_err;
use super::helpers::TrackerState;
use super::params::RawParams;
use crate::peers::{bencode_scrape, hex, percent_decode};
use actix_web::{get, web, HttpResponse};
use std::sync::atomic::Ordering;

#[get("/scrape/{passkey}")]
pub(crate) async fn scrape(
    state: web::Data<TrackerState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let passkey = path.into_inner();
    state.metrics.scrape_total.fetch_add(1, Ordering::Relaxed);
    let ip = req
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "0.0.0.0".into());

    state.refresh_guard().await;
    if let Some(reason) = state.ip_banned(&ip) {
        return bencode_err(&format!("IP 已被封禁：{reason}"));
    }
    if let Some(msg) = state.rate_limited_ip(&ip).await {
        return bencode_err(msg);
    }
    if state.resolve_passkey_cached(&passkey).await.is_none() {
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
