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
    if let Some(msg) = state.rate_limited_scrape(&ip).await {
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
    // BEP48：info_hash 可重复出现多次，每次为 20 字节 percent-encoding。
    // 审计 10-07 P2：旧版不限量不去重——实测单个 GET 能塞 1039 个 hash
    // （8KB 请求换 70KB 响应），外置模式下每条还是一次 HGETALL；
    // 重复 hash 又会在 files 字典里产出**重复键**（非合规 bencode）。
    let cap = super::guard_store::scrape_max_hashes();
    let mut seen: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let mut files: Vec<(Vec<u8>, usize, usize, usize)> = Vec::new();
    for pair in req.query_string().split('&') {
        if files.len() >= cap {
            break;
        }
        let mut it = pair.splitn(2, '=');
        if it.next() == Some("info_hash") {
            let raw = percent_decode(it.next().unwrap_or("").as_bytes());
            if raw.len() == 20 {
                let hexkey = hex(&raw);
                if !seen.insert(hexkey.clone()) {
                    continue; // 同一 hash 只回一条
                }
                // 白名单（审计 10-06 第 3 条）：未注册种子计数恒 0，
                // 不暴露随机探测的命中/未命中差异之外的任何 swarm 信息。
                let registered =
                    state.torrent_registered_scrape(&hexkey).await;
                let (s, l) = if !registered {
                    (0, 0)
                } else if crate::peers::external::external_enabled() {
                    let mut r = state.redis.clone();
                    crate::peers::external::counts(&mut r, &hexkey).await
                } else {
                    state.peers.counts(&hexkey)
                };
                // 完成数（BEP3 `downloaded`）：旧版恒写 0，客户端「完成」列永远空，
                // 而站点侧 times_completed 早就是权威值（UNIT3D/NexusPHP/Ocelot 均下发）
                let done = if registered {
                    state.scrape_completed(&hexkey)
                } else {
                    0
                };
                files.push((raw, s, l, done));
            }
        }
    }
    // bencode 字典按键的原始字节序（BEP3）；排序同时让重复键不可能出现
    files.sort_by(|a, b| a.0.cmp(&b.0));
    HttpResponse::Ok()
        .content_type("text/plain")
        .body(bencode_scrape(&files))
}
