//! HTTP announce 主链路。
//! 从 tracker main.rs 按域拆出。
use actix_web::{get, web, HttpResponse};
use std::sync::atomic::Ordering;

use super::emit::{bencode_err, emit_agent_block, emit_event};
use super::helpers::TrackerState;
use super::params::RawParams;

use crate::peers::{bencode_announce, hex, Peer, PeerKey, CONN_UNTESTED};

#[get("/announce/{passkey}")]
pub(crate) async fn announce(
    state: web::Data<TrackerState>,
    path: web::Path<String>,
    req: actix_web::HttpRequest,
) -> HttpResponse {
    let passkey = path.into_inner();
    state.metrics.announce_total.fetch_add(1, Ordering::Relaxed);
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
    // 安全（P2）：不信任客户端自报 IP —— 仅显式配置代理时才采用参数值。
    // 0225 G30-B3 补 XFF：tracker 挂 LB 后 peer IP 全成 LB 地址（做种计费与
    // 在线数直接错）。优先级：TRUST_PROXY=1 时 XFF 首值 > TRUST_PROXY_IP=1
    // 时 ?ip= 参数 > socket 对端；XFF 取链路首值（最接近真实客户端），
    // 反代必须追加而非覆盖。
    let trust_xff =
        std::env::var("TRUST_PROXY").unwrap_or_default() == "1";
    let trust_param_ip =
        std::env::var("TRUST_PROXY_IP").unwrap_or_default() == "1";
    let ip = if trust_xff {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                req.peer_addr().map(|a| a.ip().to_string())
            })
    } else if trust_param_ip {
        params
            .get_str("ip")
            .or_else(|| req.peer_addr().map(|a| a.ip().to_string()))
    } else {
        req.peer_addr().map(|a| a.ip().to_string())
    }
    .unwrap_or_else(|| "0.0.0.0".into());

    // ⓪ 应急熔断（默认关闭，见 ANN_RATE_GLOBAL_PER_MIN）
    if state.global_shed().await {
        state
            .metrics
            .announce_global_shed
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err("tracker 负载保护已触发，请稍后再试");
    }

    // ⓪ 防护缓存热身（内部 60s 节流，命中缓存时近乎零开销）
    state.refresh_guard().await;

    // ⓪ IP 封禁（内存缓存，高频路径零 DB 开销）
    if let Some(reason) = state.ip_banned(&ip) {
        state
            .metrics
            .announce_ip_banned
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err(&format!("IP 已被封禁：{reason}"));
    }
    // ⓪' 每 IP 频率（在 passkey 之前拦垃圾流量，防无效 passkey 洪水打缓存/DB）
    if let Some(msg) = state.rate_limited_ip(&ip).await {
        state
            .metrics
            .announce_limited_ip
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err(msg);
    }

    // ① passkey → user_id + 管理开关（内存缓存 60s，命中免查 PG）
    let Some((user_id, download_enabled, suspended)) =
        state.resolve_passkey_cached(&passkey).await
    else {
        state
            .metrics
            .announce_auth_fail
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err("passkey 无效，请在站点重置");
    };
    if suspended {
        return bencode_err("账号已挂起，请联系管理组");
    }
    if !download_enabled && left > 0 {
        return bencode_err("下载权限已被禁用，请联系管理组");
    }

    // ①' 每用户频率（按 user_id 而非 IP —— NAT 场景按 IP 会误伤）
    if let Some(msg) = state.rate_limited_user(user_id).await {
        state
            .metrics
            .announce_limited_user
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err(msg);
    }

    // ①'' 客户端黑名单（G-06 / P0-7）：UA 与 peer_id 双正则交叉匹配；
    // 执行闭环（0069）：命中事件经 Redis 去重（每 user+agent 1h 一条）后投递 worker 落 cheat_events，
    // 管理组在后台可查 —— 不再是"拒绝即止、无处留痕"。
    let peer_id_readable = String::from_utf8_lossy(&peer_id_raw).into_owned();
    // 审计修复（P1）：BT 客户端把 UA 放 HTTP 头而非 query 参数 —— 旧版只读 ?agent=，
    // agent_rules 的 UA 正则恒不命中、snatches.agent 恒空。改为 UA 头优先、query 兜底。
    let agent_str = req
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| params.get_str("agent"))
        .unwrap_or_default();
    if let Some(reason) =
        state.agent_blocked(Some(agent_str.as_str()), &peer_id_readable)
    {
        state
            .metrics
            .announce_agent_blocked
            .fetch_add(1, Ordering::Relaxed);
        emit_agent_block(&state.redis, user_id, &agent_str, &ip, &reason).await;
        return bencode_err(&reason);
    }

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
            ip: ip.clone(),
            port,
            uploaded,
            downloaded,
            left,
            last_seen: chrono::Utc::now(),
            user_id,
            connectable: CONN_UNTESTED, // upsert 内部会保留既有测量值
        });
    }

    // ④ 事件投递（fire-and-forget，失败仅告警；ip/conn 供 worker 反作弊分析）
    emit_event(
        &state.redis,
        &info_hash_hex,
        user_id,
        uploaded,
        downloaded,
        event,
        left,
        &ip,
        state.peers.connectable_of(&key),
        &agent_str,
    )
    .await;

    // ③ compact 二进制响应（interval 按 site_settings.announce_interval 下发；BEP-7 v6 进 peers6）
    let (interval, min_interval) = state.intervals();
    let body = if event == "stopped" {
        bencode_announce(0, 0, 0, &[], &[], interval, min_interval)
    } else {
        let seeders = state.peers.count_seeders(&info_hash_hex);
        let leechers = state.peers.count_leechers(&info_hash_hex);
        let snap = state.peers.snapshot(&info_hash_hex, numwant, &key.peer_id);
        bencode_announce(
            seeders as i64,
            leechers as i64,
            0,
            &snap.v4,
            &snap.v6,
            interval,
            min_interval,
        )
    };
    HttpResponse::Ok().content_type("text/plain").body(body)
}
