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
            0,
            0,
            0,
            &[],
            &[],
            interval,
            min_interval,
            compact,
        );
    }
    // 全站同一个 interval 会让全体客户端同秒重发（惊群，峰值正好压在限流窗
    // 与 PG 上）。这里下发按 (种子, 账号) 稳定抖动后的值：同一客户端不会
    // 每次拿到不同值（客户端据此排程，值跳变等于自己制造重发），
    // 不同 peer 之间彼此错开。chihaya 为此专门有 interval_variation 中间件。
    // 最短间隔的裁决在 announce 入口（gate::peer_interval 同一口径），
    // 这里只负责编码。
    let interval = super::limits::peer_interval(state, info_hash, self_user);
    if crate::peers::external::external_enabled() {
        let mut r = state.redis.clone();
        let (seeders, leechers) =
            crate::peers::external::counts(&mut r, info_hash).await;
        let snap = crate::peers::external::snapshot(
            &mut r, info_hash, numwant, self_user,
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
