//! HTTP announce 主链路。
//! 从 tracker main.rs 按域拆出。
use actix_web::{get, web, HttpResponse};
use std::sync::atomic::Ordering;

use super::emit::{bencode_err, emit_agent_block, emit_event, emit_xreport};
use super::helpers::{client_ip, TrackerState};
use super::params::RawParams;

use crate::peers::{hex, PeerKey};

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
    // 审计 10-07 P2-10：port/uploaded/downloaded 与 left 同口径严格解析。
    // 旧写法走 get_i64 的「解析失败回落默认值」，`uploaded=9999...`（超 i64）
    // 会被静默当 0 —— 客户端累计读数被清空，还会让 ledger_guard 误判
    // 「读数回退」而记一条 counter_reset 作弊留痕。缺失仍按 0（stopped 允许）。
    let (port, uploaded, downloaded) = match params.announce_nums() {
        Ok(v) => v,
        Err(resp) => return resp,
    };
    // ZT81（2026-10-02）：left 缺失/负数不再静默按 0 处理。原实现缺 left 即判为
    // 做种（is_seeder = left==0），会把畸形 announce 计入 seeders 并喂 3720s TTL，
    // 造成在线做种数虚高。BEP3 中 left 为必填。
    // 审计 10-06 第 1 条补洞：非数字（left=abc）经 get_i64 默认 0 → 同样被判种，
    // 现改为严格解析——解析失败一律回错，绝不静默当 0。
    let left = match params.get_str("left").map(|v| v.parse::<i64>()) {
        Some(Ok(v)) if v >= 0 => v,
        Some(Ok(_)) => return bencode_err("left 无效"),
        Some(Err(_)) => return bencode_err("left 无效"),
        None => return bencode_err("缺少 left"),
    };
    // numwant=0 是合法请求（客户端明确表示不要 peer 列表），不应当被抬成 1
    let numwant = params.get_i64("numwant", 50).clamp(0, 200) as usize;
    let event = params.get_str("event").unwrap_or_default();
    let event = event.as_str();
    // 客户端 IP：XFF / ?ip= / socket 对端（TRUST_PROXY 门控），scrape 同源共用。
    let ip = client_ip(&req, &params);

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

    // ① passkey → 用户快照（内存缓存 60s，命中免查 PG）
    let Some(pku) = state.resolve_passkey_cached(&passkey).await else {
        state
            .metrics
            .announce_auth_fail
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err("passkey 无效，请在站点重置");
    };
    let (user_id, class_id) = (pku.id, pku.class_id);
    if pku.suspended {
        return bencode_err("账号已挂起，请联系管理组");
    }
    if !pku.download_enabled && left > 0 {
        return bencode_err("下载权限已被禁用，请联系管理组");
    }
    // ①° 即时分享率闸门：**只拦下载**（left>0），做种永不拦——
    // 拦做种等于把要补比率的人赶出 swarm，比率只会更差。
    // 与 ratio_watch 的异步观察期并存：观察期内已处置，这里放行不重复罚。
    // 档位默认 warn（只计数），门槛现网是 ratiolimit=6 + 等级全 0，
    // 由 ratio_gate_max=1.0 截断，见 ratio_gate 模块的安全轨说明。
    if left > 0 {
        let gate = super::ratio_gate::gate_for(
            pku.class_min_ratio,
            pku.class_age_days,
        );
        match super::ratio_gate::decide(&gate, &pku.who()) {
            super::ratio_gate::Decision::Block => {
                state
                    .metrics
                    .ratio_gate_block
                    .fetch_add(1, Ordering::Relaxed);
                let (th, _) = super::ratio_gate::threshold(&gate);
                return bencode_err(&format!(
                    "分享率低于本站门槛（要求 {th:.2}），请先做种提升上传量"
                ));
            }
            super::ratio_gate::Decision::Warn => {
                state
                    .metrics
                    .ratio_gate_warn
                    .fetch_add(1, Ordering::Relaxed);
            }
            super::ratio_gate::Decision::Allow => {}
        }
    }

    // ①''' 种子白名单（P0-2）：info_hash 必须已在站内注册（过审或待审——审核期
    // 发布者要先能做种；回收站/被拒不再接受新 announce）。60s 缓存，DB 不可达
    // 时 fail-open（与 passkey 缓存同等降级纪律）。待审态对外可见性见下一段。
    if !state.torrent_registered(&info_hash_hex).await {
        state
            .metrics
            .announce_torrent_unknown
            .fetch_add(1, Ordering::Relaxed);
        return bencode_err("种子不存在或不可用（torrent unregistered）");
    }

    // ①'' 准入判定（特权端口 / 谎报 left / 待审种子可见性）见 gate::announce_gate
    let hide_peers = match super::gate::announce_gate(
        &state,
        &info_hash_hex,
        user_id,
        class_id,
        left,
        port,
        event == "stopped",
    ) {
        Ok(hide) => hide,
        Err(msg) => return bencode_err(msg),
    };
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

    // peer 落表统一走 gate::write_peer（stopped 归属校验 + 顶号拒写）。
    // 0225 G30-B12：FLUX_TRACKER_PEER_STORE=redis 时外置 Hash 为主（多副本
    // 互见），内存表仍同步维护（快照导出/降级路径不受影响）。
    if let Err(msg) = super::gate::write_peer(
        &state, &key, &ip, event, port, uploaded, downloaded, left, user_id,
    )
    .await
    {
        return bencode_err(msg);
    }

    // 交叉上报投递前的解析与校验（拆到 xreport_in，本文件只管主链路）
    let xreports =
        super::xreport_in::collect(&state, &params, &info_hash_hex, user_id);
    // ④ 事件投递（fire-and-forget，失败仅告警；ip/conn/port 供 worker 反作弊分析）。
    // 同 (user,torrent) 的**周期** announce 在合并窗内只回 peer 列表、不发事件
    // （P2-1）；started/completed/stopped 是事件语义，永不被并。
    //
    // conn 取数必须与回连探测**写数的地方**同源：启用外置存储时探测结果写的是
    // `flux:swarm:{hash}`（main.rs 探测循环），多副本下本进程的内存表可能从没
    // 测过这条 peer ⇒ 读内存表恒得「未测」，worker 的「实测不可达即不算在种」
    // 判据被静默放水。UDP 侧早就是读外置的，两条通道此前口径不一致。
    let connectable = if crate::peers::external::external_enabled() {
        let mut r = state.redis.clone();
        crate::peers::external::connectable_of(&mut r, &key).await
    } else {
        state.peers.connectable_of(&key)
    };
    let merged = event.is_empty()
        && !state.claim_event_window(user_id, &info_hash_hex).await;
    if merged {
        state
            .metrics
            .announce_evt_merged
            .fetch_add(1, Ordering::Relaxed);
    } else {
        emit_event(
            &state.redis,
            &info_hash_hex,
            user_id,
            uploaded,
            downloaded,
            event,
            left,
            &ip,
            connectable,
            &agent_str,
            port,
        )
        .await;
    }

    // ④' 交叉上报投递：独立流，worker 侧用于佐证上传者的上传量。
    // 单独一条流而非塞进 announce 事件：两类消费方（计费 / 上传佐证）
    // 生命周期与幂等口径不同，混流会让任一方的重投影响另一方。
    if !xreports.is_empty() {
        emit_xreport(&state.redis, &info_hash_hex, user_id, &xreports, &ip)
            .await;
    }

    // ③ 响应编码（区间下发 + compact/BEP3 + BEP7 v6）见 http_track::reply
    let compact = params.get_i64("compact", 1) != 0;
    let body = super::reply::announce_body(
        &state,
        &info_hash_hex,
        numwant,
        user_id,
        event == "stopped" || hide_peers,
        compact,
    )
    .await;
    HttpResponse::Ok().content_type("text/plain").body(body)
}
