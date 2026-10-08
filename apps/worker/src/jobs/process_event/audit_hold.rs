//! 计账扣量留痕（从 process_event.rs 拆出，同受 300 行门禁约束）。
//!
//! 两条留痕同为 `cheat_events` 的 upsert 聚合，只差 agent 前缀与 reason，
//! 集中在此可保证「reason 必须是固定文案」这条约束不被写歪——reason 是去重
//! 键的一部分，内嵌每次都变的数值会让 ON CONFLICT 恒不命中 → hits 聚合失效、
//! 行无限膨胀，管理端前 200 条可被「每次超一点」的攻击冲掉（告警稀释）。

use crate::jobs::announce_main::AnnounceEvent;

/// 自报上传量超出 leecher 交叉佐证上限：超出部分不入账，记 `corr:{torrent}`。
pub(super) async fn note_corr<'c>(
    exec: impl sqlx::Executor<'c, Database = sqlx::Postgres>,
    ev: &AnnounceEvent,
    torrent_id: i64,
    corr_held: i64,
) {
    let _ = sqlx::query(
        "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
         VALUES ($1, $2, $3, \
           'uncorroborated_upload（自报上传量超出 leecher 佐证上限，超出部分未入账）') \
         ON CONFLICT (user_id, agent, reason) DO UPDATE \
           SET hits = cheat_events.hits + 1, last_seen = now()",
    )
    .bind(ev.user)
    .bind(format!("corr:{torrent_id}"))
    .bind(&ev.ip)
    .execute(exec)
    .await;
    tracing::warn!(
        user = ev.user,
        torrent = torrent_id,
        corr_held,
        "自报上传超出交叉佐证上限，超出部分未入账"
    );
}

/// 增量超物理速率上限：超出部分不入账，记 `speed:{torrent}`（管理组复核后
/// 可用补量接口发还，明细走 tracing）。
pub(super) async fn note_speed<'c>(
    exec: impl sqlx::Executor<'c, Database = sqlx::Postgres>,
    ev: &AnnounceEvent,
    torrent_id: i64,
    held_up: i64,
    held_down: i64,
) {
    let _ = sqlx::query(
        "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, agent, reason) DO UPDATE \
           SET hits = cheat_events.hits + 1, last_seen = now()",
    )
    .bind(ev.user)
    .bind(format!("speed:{torrent_id}"))
    .bind(&ev.ip)
    .bind("over_ceiling（增量超物理速率上限，超出部分未入账）")
    .execute(exec)
    .await;
    tracing::warn!(
        user = ev.user,
        torrent = torrent_id,
        held_up,
        held_down,
        "增量超物理速率上限，超出部分未入账"
    );
}

/// 幽灵做种画像留痕（判据本体在 `seeding_gate::verdict`，这里只管落账）。
///
/// `reason` 文案必须与历史行**逐字一致**：它是
/// `ON CONFLICT (user_id, agent, reason)` 去重键的一部分，改一个字就会把
/// 既有 hits 计数器劈成两段。所以这里用常量拼接，而不是 `\` 续行——
/// 续行会被 rustfmt 归一成带空格的单行，那正是一次「没人同意的改名」。
/// 变化的数值（自报读数）只进 tracing，不进 reason。
const GHOST_PREFIX: &str = "ghost_seed（声称数据完整但从未下载，疑似幽灵做种";
const GHOST_SUFFIX: &str = "；跨种/二传用户可申诉）";

pub(super) async fn note_ghost<'c>(
    exec: impl sqlx::Executor<'c, Database = sqlx::Postgres>,
    ev: &AnnounceEvent,
    raw_down: i64,
    raw_up: i64,
) {
    let ratio = if raw_down > 0 {
        format!("（自报下载 {raw_down} 字节，credited 未达门槛）")
    } else {
        format!("（声称上传 {raw_up} 字节）")
    };
    let _ = sqlx::query(
        "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, agent, reason) DO UPDATE \
           SET hits = cheat_events.hits + 1, last_seen = now()",
    )
    .bind(ev.user)
    .bind(format!("ghost:{}", &ev.hash[..8.min(ev.hash.len())]))
    .bind(&ev.ip)
    .bind(format!("{GHOST_PREFIX}{ratio}{GHOST_SUFFIX}"))
    .execute(exec)
    .await;
}
