//! announce 响应编码（从 announce.rs 拆出，300 行门禁）。
//!
//! compact 二进制响应（interval 按 site_settings.announce_interval 下发；BEP-7
//! 的 v6 进 peers6）。0267：`compact=0` 时回退 BEP3 原始 peer 字典列表，供不支持
//! BEP23 的老客户端；缺省/其它值一律按 compact（现代客户端的事实标准，也是唯一
//! 被压测覆盖的路径）。`hide` = 本次不该外发任何 peer 与计数
//! （stopped 的空响应、以及待审种子对非发布者的清空响应走同一条路）。

use super::helpers::TrackerState;
use crate::peers::bencode_announce;

pub(crate) async fn announce_body(
    state: &TrackerState,
    info_hash: &str,
    numwant: usize,
    self_user: i64,
    hide: bool,
    compact: bool,
) -> Vec<u8> {
    let (interval, min_interval) = state.intervals();
    if hide {
        return bencode_announce(
            0, 0, 0, &[], &[], interval, min_interval, compact,
        );
    }
    if crate::peers::external::external_enabled() {
        let mut r = state.redis.clone();
        let (seeders, leechers) =
            crate::peers::external::counts(&mut r, info_hash).await;
        let snap = crate::peers::external::snapshot(
            &mut r,
            info_hash,
            numwant,
            self_user,
        )
        .await;
        return bencode_announce(
            seeders as i64,
            leechers as i64,
            0,
            &snap.v4,
            &snap.v6,
            interval,
            min_interval,
            compact,
        );
    }
    let seeders = state.peers.count_seeders(info_hash);
    let leechers = state.peers.count_leechers(info_hash);
    let snap = state.peers.snapshot(info_hash, numwant, self_user);
    bencode_announce(
        seeders as i64,
        leechers as i64,
        0,
        &snap.v4,
        &snap.v6,
        interval,
        min_interval,
        compact,
    )
}
