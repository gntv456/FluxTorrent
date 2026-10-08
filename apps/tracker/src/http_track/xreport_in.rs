//! announce 的交叉上报（`xreport`）入参解析（审计 2026-10-08 五轮自
//! announce.rs 拆出，300 行门禁）。判据本体在 worker 侧
//! `jobs/process_event/seeding_gate.rs`，这里只做「廉价的那一层」：
//! 目标 peer 是否真实存在于本 swarm、以及声称量是否物理可能。

use std::sync::atomic::Ordering;

use super::helpers::TrackerState;
use super::params::RawParams;

/// 收出本轮有效的 `(上传者 user_id, 声称字节数)` 列表；无效条目直接丢。
///
/// 交叉上报（P0-2 治本，2026-10-07）：leecher 声明「本轮从哪些 peer
/// 下载了多少字节」——`xreport=<peer_id_hex>:<bytes>`，可重复多段/逗号分隔。
/// 这是把「上传量全自报」变成「上传量有第三方佐证」的关键：上传者的上传额
/// 最终只认这些被 leecher 确认过的量（见 worker 侧消费）。
/// 校验在 tracker 做（廉价、peer 表内存命中）：目标 peer 必须是本 swarm 里
/// **存活且在做种**的真实 peer，且不能是自己。
pub(crate) fn collect(
    state: &TrackerState,
    params: &RawParams,
    info_hash_hex: &str,
    user_id: i64,
) -> Vec<(i64, i64)> {
    let mut xreports: Vec<(i64, i64)> = Vec::new();
    // 佐证的规模先按种子大小筛一遍：一次 announce 之间不可能传完一整颗种子，
    // 声称「我从这个 peer 下载了超过种子总量」的条目本身就是废数据。
    // （真正的额度上界在 worker 侧按佐证者自己的 credited 下载量裁，见
    // jobs/process_event/seeding_gate.rs::corr_room）
    let vouch_ceiling = super::guard_store::meta_of(info_hash_hex)
        .map(|m| m.size)
        .unwrap_or(0);
    for raw_seg in params.get_all("xreport") {
        for item in raw_seg.split(',') {
            let Some((pid_hex, bytes_s)) = item.rsplit_once(':') else {
                continue;
            };
            let Ok(bytes) = bytes_s.parse::<i64>() else {
                continue;
            };
            if bytes <= 0 {
                continue;
            }
            if vouch_ceiling > 0 && bytes > vouch_ceiling {
                state
                    .metrics
                    .announce_xreport_rejected
                    .fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let pid_norm = pid_hex.trim().to_ascii_lowercase();
            if pid_norm.is_empty() {
                continue;
            }
            if let Some(target_uid) = state.peers.corroboration_target(
                info_hash_hex,
                &pid_norm,
                user_id,
            ) {
                xreports.push((target_uid, bytes));
            }
        }
    }
    xreports
}
