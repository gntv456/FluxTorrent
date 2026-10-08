//! 探测循环任务体（0309 自 main.rs 抽出，300 行门禁：main 已在基线上）。
//!
//! 三层检测链（同文件演化，语义见 main.rs 的调用点注释归档）：
//! ① TCP + BT 握手 + bitfield（廉价，全量采样）；
//! ② piece SHA-1 抽查（昂贵，`probe_piece_ratio` 随机抽人）；
//! ③ 结果写回 peer 表（外置模式同步 Redis，多副本共读）。
//!
//! 0309 开源威胁模型：探测时机加 0~`probe_jitter_secs` 随机抖动，
//! piece 抽查对象按比例随机——两者都读 site_settings（随站私有），
//! 读上游源码无法预测任何一个站的实际探测行为。

use std::time::Duration;

use super::probes;
use crate::http_track::helpers::TrackerState;

/// 起探测循环（每 5min 一轮 + 随机抖动）。调用方在 actix rt 内 spawn。
pub(crate) fn spawn(st: actix_web::web::Data<TrackerState>) {
    actix_web::rt::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(
            super::probes::PROBE_PERIOD_SECS as u64,
        ));
        // xorshift 种子取启动时刻：不引 rand，够用且每次重启不同
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15)
            | 1;
        loop {
            tick.tick().await;
            let jitter =
                crate::http_track::probe_cfg::jitter_duration(&mut seed);
            if !jitter.is_zero() {
                tokio::time::sleep(jitter).await;
            }
            run_round(&st, &mut seed).await;
        }
    });
}

/// 单轮探测：采样 → 预算/比例 → 握手（全量）→ piece 抽查（随机子集）。
async fn run_round(st: &actix_web::web::Data<TrackerState>, seed: &mut u64) {
    // 预算随在线规模线性扩容（每 peer 每轮至少 1/8 覆盖，上限 400）：
    // 固定 50 时热门种子的绝大多数 peer 永远轮不到探测，conn 恒为
    // None（未测）被 seeding 判定直接放行——幽灵做种几乎无门槛。
    let budget = (st.peers.len() / 8).clamp(50, 400);
    let candidates = st.peers.probe_candidates();
    let sampled = probes::sample_probes(&candidates, budget);
    // piece 抽查（2026-10-08 P0-1 第三阶段）：握手+bitfield 只证明
    // 「声称有数据」，比对 piece 的 SHA-1 才能证明「数据是真的」。
    // 成本高（每 peer 传一个 piece，16KiB~4MiB），故按比例随机抽人。
    // 旧写法 idx<固定预算 = 取采样集前 N 个，位置可预测，排在后段的
    // peer 永不被抽查；随机化后抽中概率与位置无关（0309）。
    // 旧环境变量 FLUX_TRACKER_PIECE_PROBE 仍生效，作绝对值上限。
    let env_cap = st
        .piece_probe_budget
        .load(std::sync::atomic::Ordering::Relaxed);
    let ratio = crate::http_track::probe_cfg::current().piece_ratio;
    let picked = crate::http_track::probe_cfg::pick_piece_indices(
        sampled.len(),
        ratio,
        seed,
    );
    let mut inflight = tokio::task::JoinSet::new();
    for (idx, (key, probe_ip, probe_port)) in sampled.into_iter().enumerate() {
        let do_piece = env_cap > 0 && idx < env_cap && picked.contains(&idx);
        // 子种子：从父流抽取（每个 probe_one 独立、不可预测）
        let sub_seed = crate::http_track::probe_cfg::cheap_rand(seed);
        let st2 = st.clone();
        inflight.spawn(async move {
            probe_one(&st2, key, probe_ip, probe_port, do_piece, sub_seed)
                .await;
        });
    }
    // 等本轮全部探测收敛（每个探测自身 3s 超时，JoinSet 并发执行，
    // 在飞数量即预算上限 400，不会打爆 fd）
    while inflight.join_next().await.is_some() {}
}

/// 单 peer 探测，按「能验证到什么程度」分档（2026-10-08 改）：
///
///   ① 纯 TCP 可达 → 不通则 **DEAD**（实锤不可信，唯一无争议的否决条件）
///   ② BT 协议握手 + bitfield → 不响应则 **SUSPECT**（无法验证，**不判不可信**）
///   ③ piece SHA-1（抽样）→ 哈希不符则 **DEAD**（实锤伪造数据）
///   通过（或 ③ 未验成）→ **OK**
///
/// 为什么要分档（通用 PT 站点的硬约束）：私有站客户端基线已知、可以要求
/// 用户关掉「仅加密连接」；通用站必须假设用户群里有 qBittorrent
/// only-encrypted、MSE-PE、peer 白名单等配置——它们**不响应明文 BT 握手**，
/// 若与「裸监听占位」同判 DEAD 就是误伤好用户、砍掉站点的做种供给。
/// DEAD 与 SUSPECT 必须在数据层分开，否则事后再也分不出这两种人。
async fn probe_one(
    st: &actix_web::web::Data<TrackerState>,
    key: super::model::PeerKey,
    probe_ip: String,
    probe_port: u16,
    do_piece: bool,
    seed: u64,
) {
    use super::model::{CONN_DEAD, CONN_OK, CONN_SUSPECT};
    // 每个 probe_one 独立种子流（spawn 后无法借用外层 &mut seed）：
    // 从父流再抽一段做子种子，子流仍不可预测
    let mut seed = seed | 1;

    // 探测超时读站点配置（开源反作弊收口）：出厂 3s/8s 是公开值，
    // 各站可调（probe_timeout_secs / probe_piece_timeout_secs）。
    let cfg = crate::http_track::probe_cfg::current();
    let tcp_bt_to = Duration::from_secs(cfg.tcp_bt_timeout_secs);
    let piece_to = Duration::from_secs(cfg.piece_timeout_secs);

    // ① 纯 TCP 可达性（最便宜，先筛）
    let tcp_ok =
        super::bt_probe::probe_tcp_reachable(&probe_ip, probe_port, tcp_bt_to)
            .await;
    if !tcp_ok {
        write_back(st, &key, CONN_DEAD).await;
        return;
    }

    // ② BT 协议验证（握手 + bitfield）。失败落 SUSPECT 而非 DEAD——
    //    端口开着却不响应明文协议，最可能是加密客户端而非作弊。
    //    peer_id 随机生成（探针身份不可识别）。
    let bt_ok = super::bt_probe::probe_bt_handshake_seeded(
        &probe_ip,
        probe_port,
        &key.info_hash,
        tcp_bt_to,
        &mut seed,
    )
    .await;
    if !bt_ok {
        write_back(st, &key, CONN_SUSPECT).await;
        return;
    }

    // ③ piece 级抽查（昂贵，仅抽样且仅对 ①② 已通过者）。
    //    抽查的 piece index 由子种子流随机决定（开源威胁模型：位置不可预测）。
    if do_piece {
        if let Some(info) =
            super::piece_cache::piece_probe_for(&st.db, &key.info_hash).await
        {
            if let Some(false) = super::bt_probe::probe_piece_hash(
                &probe_ip,
                probe_port,
                &key.info_hash,
                &info,
                piece_to,
                &mut seed,
            )
            .await
            {
                // 拿到 piece 但哈希不符 = 伪造数据实锤
                write_back(st, &key, CONN_DEAD).await;
                return;
            }
            // Some(true) = 真数据；None = 没验成 → 不判作弊，继续算可信
        }
    }
    write_back(st, &key, CONN_OK).await;
}

/// 四档结论写回（内存表 + 外置 Redis）。
///
/// 外置 Redis 仍是 bool（0225 G30-B12 的既有契约，多副本共读），因此
/// SUSPECT 在外置通道里与 DEAD 同为 false——**外置模式下无法区分「不可信」
/// 与「无法验证」**。这是已知的表达力缺口：多副本部署要精确分档，需把
/// Redis 侧升成 smallint（与 DB 侧对齐）后再改。单实例（本项目默认）不受影响。
async fn write_back(
    st: &actix_web::web::Data<TrackerState>,
    key: &super::model::PeerKey,
    state: i8,
) {
    use super::model::{CONN_DEAD, CONN_SUSPECT};
    st.peers.set_conn_state(key, state);
    if super::external::external_enabled() {
        let mut r = st.redis.clone();
        super::external::set_connectable(
            &mut r,
            key,
            state != CONN_DEAD && state != CONN_SUSPECT,
        )
        .await;
    }
}
