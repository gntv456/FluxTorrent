//! P0-1/P1 计账守卫（2026-10-07 保种组实测审计，自 process_event 抽出）。

use super::announce_main::AnnounceEvent;

/// P0-1/P1 计账守卫（2026-10-07 保种组实测审计，原 process_event 内联段抽出）：
/// ① 基线守卫——累计计数器**只应单调增长**。任何下降都视为可疑：
///   上报值 ≤ RESET_ACCEPT（1 GiB，真实重启/换机后从零起算的量级）时
///   允许重置、锚点跟随新读数；否则（读数很大却变小，如 800 GiB→0）
///   本笔增量按 0 计且**锚点钉死在旧值**，并记 cheat_events。
///   修 2026-10-07 二轮：旧实现用 `COUNTER_RESET_FLOOR=8GiB` 判「大幅回退」
///   并**不更新基线**，而 ≤8 GiB 的下降一律当正常重置、锚点下移——攻击者
///   每次把上传读数下调 8 GiB 再抬升，即可反复搬动基线无限重吃入账额度。
///   改为「下降即锁死、仅近零值可重置」后，搬基线不再可行。
/// ② 物理速率钳制——增量 ≤ 距上次上报秒数 × traffic_credit_max_bps；
///   首报（无基线）按 ≤120s 计。
/// ③ 终身天花板——单种子累计入账上传 ≤ 种子大小 × 1000。
/// 返回 `GuardOutcome`（锚点/可入账/扣留/基线标记/原始读数）。
pub(crate) async fn ledger_guard(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ev: &AnnounceEvent,
    torrent_id: i64,
    torrent_size: i64,
) -> anyhow::Result<GuardOutcome> {
    let last: Option<(i64, i64, Option<i64>, i64, i64)> = sqlx::query_as(
        "SELECT last_up, last_down, \
         EXTRACT(EPOCH FROM (now() - last_seen_at))::bigint, \
         uploaded, downloaded \
         FROM snatches WHERE user_id = $1 AND torrent_id = $2 \
         FOR UPDATE",
    )
    .bind(ev.user)
    .bind(torrent_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (last_up, last_down, acc_up, acc_down) = last
        .map(|(u, d, _, au, ad)| (u, d, au, ad))
        .unwrap_or((0, 0, 0, 0));
    // P0-1 基线守卫（保种组实测审计 2026-10-07 二轮）：客户端累计计数器只应
    // 单调增长。旧实现把「降幅 ≤ 8 GiB」当正常重置并让锚点跟随下移，等于
    // 送出一把搬基线的钥匙：每次下调 8 GiB 再抬升，就能反复重吃入账额度。
    // 新口径——**任何下降都锁死基线**，只有上报值本身已回到「接近零」
    // （真实重启/换机后计数器从 0 重新起算）才认可这次重置：
    //   · 认可：锚点跟随新读数，增量从新读数起算（仍受速率钳制）。
    //   · 不认可：锚点钉死旧值，本笔增量恒为 0，并留痕 cheat_events。
    const RESET_ACCEPT: i64 = 1024 * 1024 * 1024; // 1 GiB
    let dropped_up = last.is_some() && ev.up < last_up;
    let dropped_down = last.is_some() && ev.down < last_down;
    let reset_ok = dropped_up || dropped_down;
    // 仅当「发生下降的那一侧」读到近零值才算真实重置
    let accept_up = !dropped_up || ev.up <= RESET_ACCEPT;
    let accept_down = !dropped_down || ev.down <= RESET_ACCEPT;
    let (anchor_up, anchor_down) = if reset_ok && (accept_up && accept_down) {
        (ev.up, ev.down)
    } else {
        if reset_ok {
            let _ = sqlx::query(
                "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
                 VALUES ($1, $2, $3, 'counter_reset（累计读数回退且非近零值，疑似搬动基线刷量）') \
                 ON CONFLICT (user_id, agent, reason) DO UPDATE \
                   SET hits = cheat_events.hits + 1, last_seen = now()",
            )
            .bind(ev.user)
            .bind(format!("reset:{torrent_id}"))
            .bind(&ev.ip)
            .execute(&mut **tx)
            .await;
            tracing::warn!(
                user = ev.user,
                torrent = torrent_id,
                ev_up = ev.up,
                last_up,
                ev_down = ev.down,
                last_down,
                "累计读数回退且非近零值，本笔按 0 入账且基线保持不变"
            );
        }
        // 沿用旧锚点：增量自然为负 → max(0) → 本笔 0
        (last_up, last_down)
    };
    // 计数器回绕/客户端重置时按 0 处理
    let raw_up = (anchor_up - last_up).max(0);
    let raw_down = (anchor_down - last_down).max(0);
    // 物理可入账上限（P0-1 主防线）：增量不得超过「距上次上报秒数 × 站点声明速率」。
    // 旧实现只留痕不扣量，且 secs>=30 盲区让同秒连投的多笔大增量全额入账；
    // 首报（无基线）按一个 announce 周期计，不惩罚正常下载。
    // P2（2026-10-06 安全审计）：首报窗口从 seed_cap/2（缺省 900s）收紧。
    // 2026-10-07 二轮再从 300s 收到 120s：首报无基线、按满窗口给额度，
    // 「换 peer_id / 换种子无限首报」每颗都能吃满 window × 速率上限；
    // 120s 仍足以覆盖正常客户端首个 announce 周期内的真实增量。
    let secs = last.and_then(|(_, _, s, _, _)| s).unwrap_or(120).max(1);
    let ceiling_bps = credit_ceiling(tx).await;
    let allowance = ceiling_bps.saturating_mul(secs);
    let credit_up = raw_up.min(allowance);
    let credit_down = raw_down.min(allowance);
    // P1 终身天花板（保种组实测审计 2026-10-07）：正常做种几十年也到不了
    // 「种子大小 × 1000」的累计上传；伪造者把速率钳制贴着吃时在此二次截断。
    // 只约束上传侧（下载侧天然被 H&R/buffer 义务约束，且下载是消费不是收益）。
    let lifetime_cap = torrent_size.saturating_mul(1000);
    let credit_up = if torrent_size > 0 {
        credit_up.min((lifetime_cap - acc_up).max(0))
    } else {
        credit_up
    };
    let held_up = raw_up - credit_up;
    let held_down = raw_down - credit_down;
    let had_baseline = last.is_some();
    Ok(GuardOutcome {
        anchor_up,
        anchor_down,
        credit_up,
        credit_down,
        held_up,
        held_down,
        had_baseline,
        // 原始累计读数（未经倍率/守卫裁剪）：只用于留痕时描述现场。
        // 审计 10-07 P0 实测：全新账号一次 announce 把 downloaded 报成种子
        // 大小，就同时满足「非零」与「≥10.4%」，而同一条 snatches 行的
        // credited 下载是 0 —— 所以「有没有真下过数据」不能再用它当证据。
        raw_up: ev.up,
        raw_down: ev.down,
        // 下载侧物理证据 = 本笔 credited（速率钳后、**倍率前**，故 freeleech
        // 下也非零，不会误杀免费种的真做种者）∪ 历史 credited 下载
        // （兜住「重新做种老种子」与客户端计数器归零重来的场景）。
        phys_down: credit_down.max(acc_down),
    })
}

/// 计账守卫结论（字段化替代 10 元组：调用方按名取，可读性优先）。
pub(crate) struct GuardOutcome {
    /// 写库锚点（GREATEST 单调，杜绝任何路径把基线拉低）
    pub(crate) anchor_up: i64,
    pub(crate) anchor_down: i64,
    /// 本笔可入账量（已过速率钳 + 终身上限）
    pub(crate) credit_up: i64,
    pub(crate) credit_down: i64,
    /// 因超上限被扣留的量（留痕用）
    pub(crate) held_up: i64,
    pub(crate) held_down: i64,
    /// 此前是否已有基线（首报无基线，首报留痕不记作弊）
    pub(crate) had_baseline: bool,
    /// 客户端上报的原始累计上传/下载（未裁剪，仅供留痕描述）
    pub(crate) raw_up: i64,
    pub(crate) raw_down: i64,
    /// 下载侧物理量：本笔 credited 与历史 credited 的较大值
    pub(crate) phys_down: i64,
}

/// 单次 announce 可入账的速率上限（字节/秒）：`traffic_credit_max_bps`（0285）优先，
/// 回落 `speed_alarm_bps`（告警线），都缺省/非法时回落 128 MiB/s。
/// 夹在 [1 MiB/s, 1 TiB/s]：填 0 或非数字不得把全站流量清零，也不得放开成无上限。
///
/// 2026-10-07 二轮：旧硬回落 2 GiB/s（≈17 Gbps）远超任何家用宽带
/// （千兆满载约 125 MiB/s），等于默许 ~16× 注水——伪造者把每轮增量贴着
/// 上限报就能轻松做高 ratio。128 MiB/s 已是千兆级上限的合理上界，
/// 真需要放开（机房/数据中心做种）由后台显式配置，不靠缺省值。
pub(super) async fn credit_ceiling(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> i64 {
    const DEFAULT_BPS: i64 = 134_217_728; // 128 MiB/s
    let r = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE( \
           MAX(CASE WHEN name = 'traffic_credit_max_bps' \
                    THEN NULLIF(value, '')::bigint END), \
           MAX(CASE WHEN name = 'speed_alarm_bps' \
                    THEN NULLIF(value, '')::bigint END), \
           134217728) \
         FROM site_settings \
         WHERE name IN ('traffic_credit_max_bps', 'speed_alarm_bps')",
    )
    .fetch_optional(&mut **tx)
    .await;
    match r {
        Ok(v) => v.unwrap_or(DEFAULT_BPS).clamp(1_048_576, 1_099_511_627_776),
        Err(e) => {
            tracing::warn!(?e, "速率上限设定读取失败，回落 128 MiB/s");
            DEFAULT_BPS
        }
    }
}
