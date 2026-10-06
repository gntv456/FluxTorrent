//! scrape。
//! 从 tracker main.rs 按域拆出。

use super::emit::bencode_err;
use super::helpers::{client_ip, TrackerState};
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
    let params = RawParams::new(req.query_string());
    // 审计 10-06 第 3 条：scrape 曾只认 socket 对端（announce 走 XFF）——
    // LB 后两端口径分叉，封禁/限流对 scrape 用的还是 LB 地址。现同源共用。
    let ip = client_ip(&req, &params);

    state.refresh_guard().await;
    if let Some(reason) = state.ip_banned(&ip) {
        return bencode_err(&format!("IP 已被封禁：{reason}"));
    }
    if let Some(msg) = state.rate_limited_ip(&ip).await {
        return bencode_err(msg);
    }
    let Some((_uid, _de, suspended)) =
        state.resolve_passkey_cached(&passkey).await
    else {
        return bencode_err("passkey 无效");
    };
    if suspended {
        return bencode_err("账号已挂起，请联系管理组");
    }
    // BEP48：info_hash 可重复出现多次，每次为 20 字节 percent-encoding
    let mut files: Vec<(Vec<u8>, usize, usize)> = Vec::new();
    for pair in req.query_string().split('&') {
        let mut it = pair.splitn(2, '=');
        if it.next() == Some("info_hash") {
            let raw = percent_decode(it.next().unwrap_or("").as_bytes());
            if raw.len() == 20 {
                let hexkey = hex(&raw);
                // 白名单（审计 10-06 第 3 条）：未注册种子计数恒 0，
                // 不暴露随机探测的命中/未命中差异之外的任何 swarm 信息。
                let (s, l) = if !state.torrent_registered_scrape(&hexkey).await
                {
                    (0, 0)
                } else if crate::peers::external::external_enabled() {
                    let mut r = state.redis.clone();
                    crate::peers::external::counts(&mut r, &hexkey).await
                } else {
                    state.peers.counts(&hexkey)
                };
                files.push((raw, s, l));
            }
        }
    }
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(bencode_scrape(&files))
}
