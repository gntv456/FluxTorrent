//! UDP scrape 处理（BEP15）：connection_id 校验、passkey 鉴权与计数响应编码。

use std::net::SocketAddr;
use std::sync::atomic::Ordering;

use super::core::UdpTracker;
use super::pkt::{passkey_from_tracker_id, SCRAPE_ACTION};

pub(super) async fn scrape(
    t: &UdpTracker,
    pkt: &[u8],
    peer: SocketAddr,
    transaction_id: u32,
) -> Vec<u8> {
    let conn_id = u64::from_be_bytes(pkt[0..8].try_into().unwrap());
    if !t.check_conn(&peer, conn_id) {
        return UdpTracker::err_pkt(
            transaction_id,
            "connection_id 无效或过期，请重连",
        );
    }
    // ZT81（2026-10-02）修复：原实现用 `(pkt.len()-16)/20` 整除推断 hash 个数，对
    // 「1 个 hash + 32 字节 passkey」（整包 68B）会算出 2 个 hash，把 passkey 前 20
    // 字节当成第二个 info_hash、尾段只剩 12 字节 → **真实 32 字符 passkey 恒被拒**。
    // 改为「先扣掉 site passkey（CHAR(32)）再按 20 对齐」；既无尾随 passkey 的
    // 标准 BEP15 报文则明确回错引导走 HTTP（UDP 无身份，绝不静默放行）。
    const PASSKEY_LEN: usize = 32;
    let body = pkt.len().saturating_sub(16);
    let n_hash = if body >= 20 + PASSKEY_LEN && (body - PASSKEY_LEN) % 20 == 0 {
        (body - PASSKEY_LEN) / 20
    } else {
        return UdpTracker::err_pkt(
            transaction_id,
            "UDP scrape 需在 info_hash 列表后附带 passkey；\
             标准 BEP15 匿名刮擦请改用 HTTP scrape",
        );
    };
    if n_hash == 0 {
        return UdpTracker::err_pkt(transaction_id, "scrape 需附带 info_hash");
    }
    t.state.metrics.scrape_total.fetch_add(1, Ordering::Relaxed);
    // 审计修复（P2 口径对齐）：HTTP scrape 需 passkey 鉴权，UDP 侧此前仅凭
    // connection_id 即可刮擦（connect 无身份）。要求 hash 块之后尾随 passkey
    // （与 announce 同约定：整包 = 头16 + N×20 hash + passkey）。
    let passkey_raw = String::from_utf8_lossy(&pkt[16 + n_hash * 20..]);
    let passkey = passkey_from_tracker_id(&passkey_raw);
    if passkey.len() < 16 {
        return UdpTracker::err_pkt(
            transaction_id,
            "scrape 需在 info_hash 列表后附带 passkey",
        );
    }
    let Some(pku) = t.state.resolve_passkey_cached(passkey).await else {
        return UdpTracker::err_pkt(transaction_id, "passkey 无效");
    };
    let (uid, class_id) = (pku.id, pku.class_id);
    if pku.suspended {
        return UdpTracker::err_pkt(transaction_id, "账号已被挂起");
    }
    let mut out = Vec::with_capacity(8 + n_hash * 12);
    out.extend_from_slice(&SCRAPE_ACTION.to_be_bytes());
    out.extend_from_slice(&transaction_id.to_be_bytes());
    // 条数上限（审计 10-07 P2）：与 HTTP scrape 同档，超限只回前 N 条。
    // 注意 passkey 的切片偏移仍按**原始** n_hash 算，截断只影响回包条数。
    let cap = crate::http_track::guard_store::scrape_max_hashes();
    let n_show = n_hash.min(cap);
    for chunk in pkt[16..16 + n_show * 20].chunks_exact(20) {
        let hexkey = crate::peers::hex(chunk);
        // 白名单（审计 10-06 第 3 条）：与 HTTP scrape 同口径，未注册种子计 0；
        // 待审种子对局外人同样归零（五轮实测：这个口此前完全不看档位）。
        let registered = t.state.torrent_registered_scrape(&hexkey).await
            && !crate::http_track::guard_store::hides_pending(
                &hexkey, uid, class_id,
            );
        let (s, l) = if !registered {
            (0, 0)
        } else if crate::peers::external::external_enabled() {
            let mut r = t.state.redis.clone();
            let (s, l) = crate::peers::external::counts(&mut r, &hexkey).await;
            (s as i32, l as i32)
        } else {
            let (s, l) = t.state.peers.counts(&hexkey);
            (s as i32, l as i32)
        };
        let done = if registered {
            t.state.scrape_completed(&hexkey) as i32
        } else {
            0
        };
        // BEP15 的 scrape 响应每格顺序是 **complete, downloaded, incomplete**
        // （不是 HTTP scrape 的 complete/incomplete/downloaded 字典！）。
        // 五轮实测（对照设成 77 的 times_completed 与在场 leecher 数）确认
        // 旧写法把第 2、3 格发了 (incomplete, downloaded) —— 与规范正好对调，
        // 标准客户端读到的「完成数」其实是「下载中人数」。
        out.extend_from_slice(&s.to_be_bytes());
        out.extend_from_slice(&done.to_be_bytes()); // BEP15 downloaded = 完成数
        out.extend_from_slice(&l.to_be_bytes()); // BEP15 incomplete = 下载中
    }
    out
}
