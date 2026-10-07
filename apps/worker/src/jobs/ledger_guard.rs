//! P0-1/P1 计账守卫（2026-10-07 保种组实测审计，自 process_event 抽出）。

use super::announce_main::AnnounceEvent;

/// P0-1/P1 计账守卫（2026-10-07 保种组实测审计，原 process_event 内联段抽出）：
/// ① 基线重置守卫——客户端累计读数大幅回退（>8 GiB，远超任何真实回绕）时
///   本笔增量按 0 计且**不更新基线**（实测「报 i64MAX → 报 0 → 再报」可把
///   last_up 任意搬动无限重吃入账额度）；≤8 GiB 的回退视为客户端重置的正常
///   场景，锚点跟随新读数（受速率钳制约束，敞口有限）。
/// ② 物理速率钳制——增量 ≤ 距上次上报秒数 × traffic_credit_max_bps；
///   首报（无基线）按 ≤300s 计。
/// ③ 终身天花板——单种子累计入账上传 ≤ 种子大小 × 1000。
/// 返回 (写库锚点 up/down, 可入账 up/down, 距上次秒数, 被扣留 up/down)。
#[allow(clippy::type_complexity)]
pub(crate) async fn ledger_guard(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ev: &AnnounceEvent,
    torrent_id: i64,
    torrent_size: i64,
    seed_cap: i64,
) -> anyhow::Result<(i64, i64, i64, i64, bool, i64, i64)> {
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
    let (last_up, last_down, acc_up, _acc_down) = last
        .map(|(u, d, _, au, ad)| (u, d, au, ad))
        .unwrap_or((0, 0, 0, 0));
    // P0-1 基线重置守卫（保种组实测审计 2026-10-07）：客户端累计计数器只应
    // 单调增长（真实回绕幅度极小：32 位客户端从 2^32 附近回 0）。实测
    // 「报 i64MAX → 报 0 → 再报」可把 last_up 基线任意搬动无限重吃入账额度；
    // 反向把基线钉死在高位还能让真实上传永不入账（最小化 H&R 义务）。
    // 守卫：大幅回退（降幅 > 8 GiB，远超任何真实回绕）时本笔增量按 0 处理
    // 且**不更新基线**（沿用旧锚点），并记 cheat_events 供管理组复核。
    // 回绕容错：client 重启/换机导致读数清零是真实场景，累计值 < 8 GiB 的
    // 回退视为正常重置（此时旧基线也一并失效，增量按新读数全量起算——
    // 数额有限且受速率钳制约束，接受该小敞口换取不误伤真实用户）。
    const COUNTER_RESET_FLOOR: i64 = 8 * 1024 * 1024 * 1024;
    let reset_up = last.is_some() && ev.up < last_up - COUNTER_RESET_FLOOR;
    let reset_down =
        last.is_some() && ev.down < last_down - COUNTER_RESET_FLOOR;
    // 小幅回退（≤8 GiB）＝客户端重置的真实场景：锚点直接跟随新读数
    // （GREATEST 兜底仅拦大幅回退），增量从 0 起算、受速率钳制约束。
    let (anchor_up, anchor_down) = if reset_up || reset_down {
        let _ = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
             VALUES ($1, $2, $3, 'counter_reset（累计读数大幅回退，疑似基线重置刷量）') \
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
            "累计读数大幅回退，本笔按 0 入账且基线保持不变"
        );
        // 只回退的一侧按旧锚点换算（两侧可能只有一侧异常），增量自然为负 → max(0)
        (last_up, last_down)
    } else {
        (ev.up, ev.down)
    };
    // 计数器回绕/客户端重置时按 0 处理
    let raw_up = (anchor_up - last_up).max(0);
    let raw_down = (anchor_down - last_down).max(0);
    // 物理可入账上限（P0-1 主防线）：增量不得超过「距上次上报秒数 × 站点声明速率」。
    // 旧实现只留痕不扣量，且 secs>=30 盲区让同秒连投的多笔大增量全额入账；
    // 首报（无基线）按一个 announce 周期计，不惩罚正常下载。
    // P2（2026-10-06 安全审计）：首报窗口从 seed_cap/2（缺省 900s）收紧到
    // ≤300s——旧口径下「换 peer_id/换种子无限首报」每颗种子都能吃满
    // 900s × 速率上限；300s 足以覆盖正常客户端首个 announce 周期
    // （interval 缺省 1800s 时客户端首次汇报的增量本来就该按下载启动
    // 时刻起算，900s 的宽限只便宜了伪造者）。
    let secs = last
        .and_then(|(_, _, s, _, _)| s)
        .unwrap_or((seed_cap / 2).max(60).min(300))
        .max(1);
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
    Ok((
        anchor_up,
        anchor_down,
        credit_up,
        credit_down,
        had_baseline,
        held_up,
        held_down,
    ))
}

/// 单次 announce 可入账的速率上限（字节/秒）：`traffic_credit_max_bps`（0285）优先，
/// 回落 `speed_alarm_bps`（告警线），都缺省/非法时回落 2 GiB/s（与旧默认一致）。
/// 夹在 [1 MiB/s, 1 TiB/s]：填 0 或非数字不得把全站流量清零，也不得放开成无上限。
pub(super) async fn credit_ceiling(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> i64 {
    let r = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE( \
           MAX(CASE WHEN name = 'traffic_credit_max_bps' \
                    THEN NULLIF(value, '')::bigint END), \
           MAX(CASE WHEN name = 'speed_alarm_bps' \
                    THEN NULLIF(value, '')::bigint END), \
           2147483648) \
         FROM site_settings \
         WHERE name IN ('traffic_credit_max_bps', 'speed_alarm_bps')",
    )
    .fetch_optional(&mut **tx)
    .await;
    match r {
        Ok(v) => v
            .unwrap_or(2_147_483_648)
            .clamp(1_048_576, 1_099_511_627_776),
        Err(e) => {
            tracing::warn!(?e, "速率上限设定读取失败，回落 2 GiB/s");
            2_147_483_648
        }
    }
}
