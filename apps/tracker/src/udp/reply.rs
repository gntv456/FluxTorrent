//! UDP announce 的响应构造：在线计数与 peer 快照取数 + BEP 15 字节打包。
//! 从 announce.rs 拆出（300 行门禁），语义与 HTTP 侧 reply::announce_body 对齐。

use actix_web::web;

use crate::peers::Snapshot;

/// 本次响应要下发的 `(seeders, leechers, 快照)`。
///
/// `stopped` 与待审种子对局外人一律清空计数与列表：前者客户端已经不在了，
/// 后者不能让还没过审的种子对外可见。
pub(super) async fn resolve(
    state: &web::Data<crate::TrackerState>,
    info_hash: &str,
    event: &str,
    hide_peers: bool,
    numwant: usize,
    user_id: i64,
) -> (i32, i32, Snapshot) {
    if event == "stopped" || hide_peers {
        return (0, 0, Default::default());
    }
    let want = numwant.clamp(0, 200);
    if crate::peers::external::external_enabled() {
        let mut r = state.redis.clone();
        let (seeders, leechers) =
            crate::peers::external::counts(&mut r, info_hash).await;
        let snap =
            crate::peers::external::snapshot(&mut r, info_hash, want, user_id)
                .await;
        (seeders as i32, leechers as i32, snap)
    } else {
        let snap = state.peers.snapshot(info_hash, want, user_id);
        (
            state.peers.count_seeders(info_hash) as i32,
            state.peers.count_leechers(info_hash) as i32,
            snap,
        )
    }
}

/// BEP 15 的 `peers` 字段：每条 6 字节（4 IP + 2 port）紧凑拼接。
pub(super) fn pack_v4(snap: &Snapshot) -> Vec<u8> {
    let mut peers_bytes = Vec::with_capacity(snap.v4.len() * 6);
    for p in &snap.v4 {
        peers_bytes.extend_from_slice(&p.ip);
        peers_bytes.extend_from_slice(&p.port.to_be_bytes());
    }
    peers_bytes
}
