//! announce 的准入判定与 peer 落表（审计 10-07 P1-1 / P1-2 / P2-10 / P2-11
//! 自 announce.rs、guard_refresh.rs 拆出，300 行门禁）。

use std::sync::atomic::Ordering;

use super::helpers::TrackerState;
use crate::peers::{Peer, PeerKey, CONN_UNTESTED};

/// 写 peer 表：`stopped` 走归属校验的移除，其余 upsert。
/// `Err` = 该 `(info_hash, peer_id)` 槽位属于**其他账号**（顶号企图）。
/// 旧实现没有归属校验：先用他人 peer_id 发一条 announce（槽位连人带回连测量
/// 一起易主），再发一条 stopped 就能把从未停种的受害者抹出 swarm——10-06 补的
/// `remove_owned` 因此被绕过；反过来也能把受害者「回连可达」的测量值搬到攻击
/// 者身上，绕掉幽灵做种的 conn 判据（均实测复现）。
pub(crate) async fn write_peer(
    state: &TrackerState,
    key: &PeerKey,
    ip: &str,
    event: &str,
    port: u16,
    uploaded: i64,
    downloaded: i64,
    left: i64,
    user_id: i64,
) -> Result<(), &'static str> {
    if event == "stopped" {
        if crate::peers::external::external_enabled() {
            let mut r = state.redis.clone();
            let _ = crate::peers::external::remove(&mut r, key, user_id).await;
        }
        state.peers.remove_owned(key, user_id);
        return Ok(());
    }
    let peer = Peer {
        key: key.clone(),
        ip: ip.to_string(),
        port,
        uploaded,
        downloaded,
        left,
        last_seen: chrono::Utc::now(),
        user_id,
        connectable: CONN_UNTESTED, // upsert 内部保留同账号的既有测量值
    };
    if crate::peers::external::external_enabled() {
        let mut r = state.redis.clone();
        let _ = crate::peers::external::upsert(&mut r, peer.clone()).await;
    }
    if !state.peers.upsert(peer) {
        state
            .metrics
            .announce_peer_taken
            .fetch_add(1, Ordering::Relaxed);
        return Err(
            "peer_id 与本站其他账号冲突，请在客户端重置 peer_id 后重新下载种子",
        );
    }
    Ok(())
}

/// announce 准入判定，一次给全结论（策略判断不散进 announce.rs）：
///   · 特权端口（<1024 且非 stopped）→ Err（UNIT3D `BLACK_PORTS`、NexusPHP
///     `portblacklisted()`：这类端口不是真实 BT 监听口，却会被下发给同 swarm
///     的其他客户端，也成为 tracker 主动回连探测的目标）
///   · `left > 种子大小` → Err（NexusPHP 判 fake announce；谎报 left 可刷
///     progress、绕 TTL 分档，与幽灵做种同源）
///   · 待审种子（approval=0）+ 非发布者 + 非 staff → Ok(true)：接受 announce
///     （发布者审核期要能做种），但对外的 peer 列表与计数清空。
///     策略见 `announce_pending_policy`（allow_all / self_seed_only / owner_only）。
pub(crate) fn announce_gate(
    state: &TrackerState,
    info_hash: &str,
    user_id: i64,
    class_id: i32,
    left: i64,
    port: u16,
    stopped: bool,
) -> Result<bool, &'static str> {
    if port != 0 && port < 1024 && !stopped {
        return Err("port 无效（特权端口）");
    }
    // 服务端口黑名单（五轮补齐的另一半：UNIT3D `BLACK_PORTS` /
    // NexusPHP `portblacklisted()`）
    if super::limits::port_blacklisted(port) && !stopped {
        return Err("port 在本站黑名单内（不是可用的 BT 监听端口）");
    }
    if let Some(meta) = super::guard_store::meta_of(info_hash) {
        if meta.size > 0 && left > meta.size {
            state
                .metrics
                .announce_fake_left
                .fetch_add(1, Ordering::Relaxed);
            return Err("left 超过种子大小（fake announce）");
        }
        let outsider =
            meta.approval == 0 && meta.owner_id != user_id && class_id < 90;
        if !outsider {
            return Ok(false);
        }
        return match super::guard_store::pending_policy() {
            0 => Ok(false),
            2 => Err("种子审核中，暂不可访问"),
            _ => Ok(true),
        };
    }
    Ok(false)
}

/// 事件合并窗比例（`ANN_EVENT_MERGE_PCT`，默认 40%；0 = 关闭合并）。
pub(crate) fn event_merge_pct() -> i64 {
    static V: std::sync::OnceLock<i64> = std::sync::OnceLock::new();
    *V.get_or_init(|| {
        super::helpers::env_i64("ANN_EVENT_MERGE_PCT", 40).clamp(0, 90)
    })
}

/// 合并窗长度（秒）；`pct <= 0` ⇒ 0 = 不合并。
///
/// 为什么并得掉：一条事件 = 一个 PG 事务 + `FOR UPDATE` 行锁，而计账口径是
/// 「客户端累计读数 − 上次锚点」，做种时长按相邻事件的时间差累计——相邻两条
/// 合并成一条，流量与时长一分不差（差值口径的前提）。
/// 下限 60s：interval 很短时不该压成「几乎不发事件」；上限 900s：再长就丢活跃度
/// 读数（面板「正在下载」按 updated_at 排，站长看到的粒度不能太粗）。
pub(crate) fn merge_window_secs(interval: i64, pct: i64) -> i64 {
    if pct <= 0 {
        return 0;
    }
    interval.saturating_mul(pct).div_euclid(100).clamp(60, 900)
}

#[cfg(test)]
mod tests {
    /// 该模块的两个函数都要靠 TrackerState（含 Redis/PG 句柄），无法在单测里
    /// 构造；这里只钉住策略档位常量的语义，防止默认值被无意改掉。
    use super::merge_window_secs;
    use crate::http_track::guard_store::{pending_policy, set_pending_policy};

    #[test]
    fn default_policy_hides_pending_swarm() {
        // 静态量跨用例共享 ⇒ 先记后恢复，别污染其它测试
        let keep = pending_policy();
        set_pending_policy(1);
        assert_eq!(pending_policy(), 1, "默认应是 self_seed_only");
        set_pending_policy(0);
        assert_eq!(pending_policy(), 0);
        set_pending_policy(2);
        assert_eq!(pending_policy(), 2);
        set_pending_policy(keep);
    }

    #[test]
    fn merge_window_matches_interval_and_clamps() {
        // 默认 40%：1800s ⇒ 720s（正常客户端 30min 一轮，永不触窗）
        assert_eq!(merge_window_secs(1800, 40), 720);
        // 关闭档：0 = 每条都发事件
        assert_eq!(merge_window_secs(1800, 0), 0);
        // 下限 60s：interval 被配得很短时也不能「几乎不发」
        assert_eq!(merge_window_secs(60, 40), 60);
        // 上限 900s：interval 配到 1h 时窗不该拖到 24min（活跃度粒度）
        assert_eq!(merge_window_secs(3600, 40), 900);
        // 负数 interval（脏配置）不该 panic
        assert_eq!(merge_window_secs(-10, 40), 60);
    }
}
