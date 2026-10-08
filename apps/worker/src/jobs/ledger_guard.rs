//! P0-1/P1 计账守卫（2026-10-07 保种组实测审计，自 process_event 抽出）。

use super::announce_main::AnnounceEvent;

/// 近零读数才算「客户端真的重启了」。六轮审计（2026-10-08）从 1 GiB 收紧到
/// **16 MiB**：真重启后几分钟内重新累积的量级是 0~几 MiB，而 1 GiB 的口子
/// 实测构成铸币链——「报 ≤1 GiB 重置锚点 → 等速率窗 → 报增量」每循环可再铸
/// ≤ 速率钳 × 时窗（128 MiB/s × 120s ≈ 15.6 GiB），详见报告 §九 P0-1。
pub(crate) const RESET_ACCEPT: i64 = 16 * 1024 * 1024; // 16 MiB

/// 近零重置的频次窗（秒）：窗内第二次起不被认可（真实用户一年重启几次；
/// 短窗内反复「重置 → 吃增量」是搬基线攻击的节奏指纹）。判定见
/// [`ledger_guard`] 对 `snatches.reset_last_at` 的检查。
pub(crate) const RESET_WINDOW_SECS: i64 = 24 * 3600;

/// 锚点判定结论（见 [`anchor_for`]）。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Anchor {
    /// 写回 `snatches.last_up / last_down` 的新锚点
    pub(crate) up: i64,
    pub(crate) down: i64,
    /// 本侧刚认可了一次「近零重置」⇒ 调用方必须让锚点**下调**
    /// （SQL 侧的 `GREATEST` 兜底要把这一格让路，否则真重启的用户
    /// 第二次 announce 永远追不回锚点 = 流量永久冻结，五轮实测发现）
    pub(crate) reset_up: bool,
    pub(crate) reset_down: bool,
    /// 读数下降但新值仍不接近零 ⇒ 搬基线企图，锚点钉死 + 留痕
    pub(crate) refused: bool,
}

/// 锚点判定（纯函数，单测覆盖）：给定本次上报的累计读数与上次锚点，
/// 决定写回的新锚点。三种情形必须分清楚：
///
/// ① **单调增长（正常路径）** ⇒ 锚点跟随新读数，增量 = 新读数 − 旧锚点。
///    2026-10-08 五轮实测：批一把这条和③并进了同一个 else 分支
///    （判据写成 `reset_ok && accept`，正常上报时 `reset_ok` 恒 false），
///    于是 `raw_up = (anchor - last) = 0` —— 全站流量入账停摆，并且
///    `phys_down` 恒 0 把所有真做种者打成 ghost_seed（cheat_enforce
///    实测 L2 告警 hits=70）。**判据的否定式分支必须自带反例单测。**
/// ② **下降且新值 ≤ RESET_ACCEPT** ⇒ 认可为真实重启：锚点跟随新读数，
///    本笔增量按 0 计（增量从新读数起算要到下一次 announce）。
///    **两侧必须同时近零**（六轮审计 P0-1）：真实客户端重启 up/down 计数器
///    一起归零；只归零一侧（尤其只归零 up 侧）是搬基线攻击的最小动作。
///    单侧近零不认可下调，锚点钉死 + refused（与③同路）。
/// ③ **下降但新值仍很大**（如 800 GiB→0→500 GiB 这种搬基线）⇒ 锚点钉死
///    在旧值，本笔按 0 计，调用方记 cheat_events。
pub(crate) fn anchor_for(
    ev_up: i64,
    ev_down: i64,
    last: Option<(i64, i64)>,
) -> Anchor {
    let (last_up, last_down) = last.unwrap_or((0, 0));
    // 首报（无基线）：只立基线，谈不上回退
    if last.is_none() {
        return Anchor {
            up: ev_up,
            down: ev_down,
            reset_up: false,
            reset_down: false,
            refused: false,
        };
    }
    let dropped_up = ev_up < last_up;
    let dropped_down = ev_down < last_down;
    // 六轮 P0-1：**两侧读数同时近零**才算真实重启。旧写法逐侧判断「下降的
    // 那侧是否近零」，于是 `up: 500GiB→0` 而 down 停在 20GiB 也被认可——那正是
    // 搬基线的最小动作（只把要刷的一侧打回 0）。真实客户端的 up/down 是同一
    // 进程内的一对计数器，重启必然一起归零 ⇒ 这条不误伤。
    let near_zero = ev_up <= RESET_ACCEPT && ev_down <= RESET_ACCEPT;
    let accepted_reset = (dropped_up || dropped_down) && near_zero;
    let refused = (dropped_up || dropped_down) && !accepted_reset;
    if refused {
        return Anchor {
            up: last_up,
            down: last_down,
            reset_up: false,
            reset_down: false,
            refused: true,
        };
    }
    Anchor {
        up: ev_up,
        down: ev_down,
        reset_up: dropped_up,
        reset_down: dropped_down,
        refused: false,
    }
}

/// P0-1/P1 计账守卫：① 基线/锚点（[`anchor_for`]）；② 物理速率钳制；
/// ③ 终身天花板。
///
/// ②的口径（2026-10-08 五轮加固）：增量 ≤ `min(距上次上报秒数, secs_cap)`
/// × `traffic_credit_max_bps`。**秒窗必须封上界**：secs 取自
/// `snatches.last_seen_at`，也就是由攻击者的「沉默时长」决定——旧写法只有
/// 下界，一次 announce 前静默 24h 就能把单笔额度撑到 128 MiB/s × 86400 s
/// ≈ 11.26 TiB，等于「announce 越稀疏越能吃」。secs_cap 由调用方传入的做种
/// 时长容忍窗（2 × announce_interval）决定，与 seeded_seconds 的封顶同一
/// 口径，两条判据不会互相矛盾。
///
/// 首报（无基线）：raw 恒 0，只立锚点。客户端的累计计数器是**全局**的
/// （不是每种一个），把首次 announce 的读数当增量入账等于把别的种子的量
/// 记到这颗种上；旧写法靠 120 s 窗口给它限额，仍是「换种子无限首报」的
/// 铸币口（假种审计 P1「单种首报额度」）。
///
/// ③终身天花板：单种子累计入账上传 ≤ 种子大小 × 1000（只约束上传侧——
/// 下载是消费不是收益，且天然被 H&R/buffer 义务约束）。
///
/// 返回 `GuardOutcome`（锚点/可入账/扣留/基线标记/原始读数/物理下载量）。
pub(crate) async fn ledger_guard(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ev: &AnnounceEvent,
    torrent_id: i64,
    torrent_size: i64,
    secs_cap: i64,
) -> anyhow::Result<GuardOutcome> {
    let last: Option<(
        i64,
        i64,
        Option<i64>,
        i64,
        i64,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        "SELECT last_up, last_down, \
             EXTRACT(EPOCH FROM (now() - last_seen_at))::bigint, \
             uploaded, downloaded, reset_last_at \
             FROM snatches WHERE user_id = $1 AND torrent_id = $2 \
             FOR UPDATE",
    )
    .bind(ev.user)
    .bind(torrent_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (last_up, last_down, acc_up, acc_down, reset_last_at) = last
        .map(|(u, d, _, au, ad, r)| (u, d, au, ad, r))
        .unwrap_or((0, 0, 0, 0, None));
    let mut anchor =
        anchor_for(ev.up, ev.down, last.map(|(u, d, _, _, _, _)| (u, d)));
    // P0-1 频次限制（六轮审计）：近零重置在 RESET_WINDOW_SECS 窗内只认可一次。
    // 真实用户一年重启几次；短窗内反复「重置锚点 → 吃增量」每循环可再铸
    // ≤ 速率钳 × 时窗的量（128 MiB/s × 120s ≈ 15.6 GiB，实测复现）。
    // 窗内第二次起按搬基线处理：锚点钉死 + refused 留痕。
    if (anchor.reset_up || anchor.reset_down)
        && reset_last_at.is_some_and(|t| {
            chrono::Utc::now() - t
                < chrono::Duration::seconds(RESET_WINDOW_SECS)
        })
    {
        tracing::warn!(
            user = ev.user,
            torrent = torrent_id,
            "近零重置频发（24h 窗内二次），锚点钉死"
        );
        anchor = Anchor {
            up: last_up,
            down: last_down,
            reset_up: false,
            reset_down: false,
            refused: true,
        };
    }
    let (anchor_up, anchor_down) = (anchor.up, anchor.down);
    // 认可了重置 ⇒ 让 upsert 侧把 reset_last_at 置为 now（频次窗的游标）
    let reset_accepted = anchor.reset_up || anchor.reset_down;
    if anchor.refused {
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
    // 增量 = 新锚点 − 旧锚点；首报（无基线）恒 0，只立基线
    let (raw_up, raw_down) = if last.is_some() {
        (
            (anchor_up - last_up).max(0),
            (anchor_down - last_down).max(0),
        )
    } else {
        (0, 0)
    };
    let secs = last
        .and_then(|(_, _, s, _, _, _)| s)
        .unwrap_or(120)
        .clamp(1, secs_cap.max(1));
    let ceiling_bps = credit_ceiling(tx).await;
    let allowance = ceiling_bps.saturating_mul(secs);
    let credit_up = raw_up.min(allowance);
    let credit_down = raw_down.min(allowance);
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
        reset_up: anchor.reset_up,
        reset_down: anchor.reset_down,
        // 频次窗游标：本笔认可了近零重置 ⇒ upsert 侧把 reset_last_at 置 now
        reset_accepted,
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
    /// 写库锚点（近零重置时允许下调，其余只进不退）
    pub(crate) anchor_up: i64,
    pub(crate) anchor_down: i64,
    /// 本次认可了 up/down 侧的重置（SQL 侧 GREATEST 要据此让路）
    pub(crate) reset_up: bool,
    pub(crate) reset_down: bool,
    /// 本次认可了近零重置（upsert 侧据此置 reset_last_at = now，做 24h 频次窗游标）
    pub(crate) reset_accepted: bool,
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

#[cfg(test)]
mod tests;
