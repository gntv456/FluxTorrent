//! announce 事件计费处理。
//! 从 jobs.rs 按域拆出。

use super::announce_main::AnnounceEvent;
use super::billing::billing_multipliers;
use sqlx::PgPool;

/// ZT81（2026-10-02）：按**字符边界**截断。原实现用 `&s[..len.min(200)]` 按字节
/// 切片，UA 含多字节 UTF-8 且恰好落在第 200 字节非边界时会 panic。
fn truncate_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 单事件计费（促销裁决 + snatch upsert + 流水 + 保种时长累计）
/// 返回 torrent_id 供快照点刷收集（种子不存在/重复事件返回 None）。
/// seed_cap：做种时长单次累计容忍窗（秒）= 2 × announce_interval，由 consume_announce 按站点设定算出
/// event_id：Redis 流条目 id，作「恰好入账一次」的幂等键（0285）
#[allow(clippy::too_many_arguments)]
pub(crate) async fn process_event(
    db: &PgPool,
    ev: &AnnounceEvent,
    seed_cap: i64,
    event_id: &str,
) -> anyhow::Result<Option<i64>> {
    // 未知种子的查询失败必须显式报错（重试），不能静默丢弃计费
    // 审计修复（P1）：announce 哈希是客户端「原始字节」口径；库内 info_hash 为规范化
    // 重编码口径（键序非排序的种子两者不同，此前静默丢计费）。双口径 OR 匹配。
    let torrent: Option<(i64, i64)> = sqlx::query_as(
                "SELECT id, \
         COALESCE(size, 0) FROM torrents WHERE info_hash = $1 OR raw_info_hash = $1",
    )
    .bind(&ev.hash)
    .fetch_optional(db)
    .await?;
    let Some((torrent_id, torrent_size)) = torrent else {
        return Ok(None); // 种子确实不存在：跳过
    };

    // 促销快照裁决（§5.4-⑦）——与 API 展示口径一致：同种子多条专属促销取最强档
    // （修复前 ORDER BY id DESC 只认最新一条：先挂 free 后挂 half 时计费取 half、展示取 free）
    // 裁决时点 = 事件时点 ev.ts（缺省 now）：积压重放时不再按过期后的价目补计费
    let ev_time = ev.ts.unwrap_or_else(chrono::Utc::now);
    // 审计修复（P0）：专属促销查询旧版把 $1 重复引用三次并 bind 三次 —— PG 扩展协议按
    // 「最大占位符编号」要求 4 个参数，但未在 SQL 中出现的编号无法推断类型，
    // Parse 阶段报 "could not determine data type of parameter $2"，每个 announce
    // 事件重试 6 次进 DLQ，计费链路整体瘫痪。改为 $1/$4 显式引用 + 仅 bind 两个参数。
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions WHERE torrent_id = $1 AND starts_at <= $2 AND ends_at > $2 \
         ORDER BY CASE kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 \
                                  WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, id DESC \
         LIMIT 1",
    )
    .bind(torrent_id)
    .bind(ev_time)
    .fetch_optional(db)
    .await?;
    // 审计修复（P0 真根因，PG 日志实锄）：旧 SQL 里 $1 出现 3 次（三个 EXISTS 子查询），
    // Rust 侧只 bind 1 个参数 —— sqlx Describe/Bind 参数计数协商失败后发出 0 参数 Bind，
    // "supplies 0 parameters" 每轮必炸，announce 计费自 07-11 起整体瘫痪。改写为 $1 单次引用。
    let global: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions p \
         WHERE p.torrent_id IS NULL AND p.starts_at <= $4 AND p.ends_at > $4 \
           AND (p.scope = 'global' \
                OR (p.scope = 'official' AND (SELECT official_tag FROM torrents WHERE id = $1)) \
                OR (p.scope = 'non_official' AND NOT (SELECT official_tag FROM torrents WHERE id = $2)) \
                OR (p.scope = 'category' AND p.category_id = (SELECT category_id FROM torrents WHERE id = $3))) \
         ORDER BY CASE kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 \
                                  WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC \
         LIMIT 1",
    )
    // 同值三占位符 + 三 bind（sqlx 按占位符种类计数；缺 bind 会 0 参数发送）
    .bind(torrent_id)
    .bind(torrent_id)
    .bind(torrent_id)
    .bind(ev_time)
    .fetch_optional(db)
    .await?;
    let (up_mult, down_mult) =
        billing_multipliers(kind.as_deref(), global.as_deref());

    // 0073 券倍率叠加：free 券 → 下载计 0；neutral 券 → 上下行均计 0。
    // 判定口径：本人该种存在绑定中（used_at 仍 NULL）的对应 kind 券；过期判定同促销用事件时点。
    // 与促销取更优（乘法叠加：促销 x2 上传对 neutral 也归零，取对用户更优的 0）。
    let voucher: Option<String> = sqlx::query_scalar(
        "SELECT kind FROM user_vouchers \
         WHERE user_id = $1::bigint AND used_torrent_id = $2::bigint AND used_at IS NULL AND expires_at > $3 \
         ORDER BY CASE kind WHEN 'neutral' THEN 2 WHEN 'free' THEN 1 ELSE 0 END DESC LIMIT 1",
    )
    .bind(ev.user)
    .bind(torrent_id)
    .bind(ev_time)
    .fetch_optional(db)
    .await?;
    let (up_mult, down_mult) = match voucher.as_deref() {
        Some("neutral") => (0.0, 0.0),
        Some("free") => (up_mult, 0.0),
        _ => (up_mult, down_mult),
    };

    // BEP3：ev.up/down 是客户端累计总量 —— 先取出上次上报值换算增量（P0 修复）
    let mut tx = db.begin().await?;

    // 恰好入账一次（P0-1／0285）：同一事件被 PEL 回收或重投时直接跳过。
    // 旧实现无幂等键，reclaim_stale(360s) 与 with_lock(900s) 超时窗重叠即双计。
    let fresh = sqlx::query(
        "INSERT INTO announce_seen (event_id, user_id) VALUES ($1, $2) \
         ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(event_id)
    .bind(ev.user)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if fresh == 0 {
        tracing::debug!(user = ev.user, torrent = torrent_id, "重复事件跳过");
        tx.commit().await?;
        return Ok(None);
    }

    let last: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT last_up, \
         last_down, \
         EXTRACT(EPOCH FROM (now() - last_seen_at))::bigint \
         FROM snatches WHERE user_id = $1 AND torrent_id = $2 FOR UPDATE",
    )
    .bind(ev.user)
    .bind(torrent_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (last_up, last_down) = last.map(|(u, d, _)| (u, d)).unwrap_or((0, 0));
    // 计数器回绕/客户端重置时按 0 处理
    let raw_up = (ev.up - last_up).max(0);
    let raw_down = (ev.down - last_down).max(0);
    // 物理可入账上限（P0-1 主防线）：增量不得超过「距上次上报秒数 × 站点声明速率」。
    // 旧实现只留痕不扣量，且 secs>=30 盲区让同秒连投的多笔大增量全额入账；
    // 首报（无基线）按一个 announce 周期计，不惩罚正常下载。
    // P2（2026-10-06 安全审计）：首报窗口从 seed_cap/2（缺省 900s）收紧到
    // ≤300s——旧口径下「换 peer_id/换种子无限首报」每颗种子都能吃满
    // 900s × 速率上限；300s 足以覆盖正常客户端首个 announce 周期
    // （interval 缺省 1800s 时客户端首次汇报的增量本来就该按下载启动
    // 时刻起算，900s 的宽限只便宜了伪造者）。
    let secs = last
        .and_then(|(_, _, s)| s)
        .unwrap_or((seed_cap / 2).max(60).min(300))
        .max(1);
    let allowance = credit_ceiling(&mut tx).await.saturating_mul(secs);
    let credit_up = raw_up.min(allowance);
    let credit_down = raw_down.min(allowance);
    let held_up = raw_up - credit_up;
    let held_down = raw_down - credit_down;
    let delta_up = (credit_up as f64 * up_mult) as i64;
    let delta_down = (credit_down as f64 * down_mult) as i64;

    // 超上限留痕：agent 沿用 cheat_audit 的 torrent:{id} 约定、speed: 前缀区分来源，
    // reason 带被扣量与证据，供管理组复核后用补量接口发还。
    if last.is_some() && (held_up > 0 || held_down > 0) {
        let _ = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, agent, reason) DO UPDATE \
               SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(ev.user)
        .bind(format!("speed:{torrent_id}"))
        .bind(&ev.ip)
        .bind(format!(
            "over_ceiling held_up={held_up} held_down={held_down} secs={secs}"
        ))
        .execute(&mut *tx)
        .await;
        tracing::warn!(
            user = ev.user,
            torrent = torrent_id,
            held_up,
            held_down,
            secs,
            "增量超物理速率上限，超出部分未入账"
        );
    }

    let seeding = ev.left == 0;
    // stopped = 客户端退出：与 tracker 侧 remove(peer) 对齐，DB 也不应继续标记在做种/下载
    let stopped = ev.event == "stopped";
    sqlx::query(
        r#"
        INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, last_up, last_down, leeching, seeding, completed_at, last_seen_at, connectable, agent, progress)
        VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $11 THEN FALSE ELSE $7 END, CASE WHEN $11 THEN FALSE ELSE $8 END, CASE WHEN $9 THEN now() ELSE NULL END, now(), COALESCE($12, 1), $13, $14)
        ON CONFLICT (user_id, torrent_id) DO UPDATE SET
          uploaded = snatches.uploaded + EXCLUDED.uploaded,
          downloaded = snatches.downloaded + EXCLUDED.downloaded,
          last_up = EXCLUDED.last_up,
          last_down = EXCLUDED.last_down,
          leeching = CASE WHEN $11 THEN FALSE ELSE EXCLUDED.leeching END,
          seeding = CASE WHEN $11 THEN FALSE ELSE EXCLUDED.seeding OR snatches.seeding END,
          completed_at = COALESCE(snatches.completed_at, EXCLUDED.completed_at),
          -- 保种时长（0043）：本次为做种 announce（left=0）且与上次 announce 间隔未超容忍窗时，
          -- 记入真实时间差；离线过久不记（防挂机伪造），停止 announce 自然停止累计，
          -- 也不受 seeding 粘性 OR 影响（以本次事件的 left 为准）
          seeded_seconds = snatches.seeded_seconds + (
              CASE WHEN EXCLUDED.seeding
                        AND snatches.last_seen_at > now() - ($10::bigint * interval '1 second')
                   THEN LEAST(GREATEST(EXTRACT(EPOCH FROM (now() - snatches.last_seen_at))::bigint, 0), $10::bigint)
                   ELSE 0 END
          ),
          -- connectable（0071）：tracker 回连抽样结果覆盖（NULL=本次未测，保持原值）
          connectable = COALESCE($12, snatches.connectable),
          -- 客户端 UA / 实时进度（0098，viewsnatches 口径）：每次 announce 覆盖
          agent = EXCLUDED.agent,
          progress = EXCLUDED.progress,
          last_seen_at = now()
        "#,
    )
    .bind(ev.user)
    .bind(torrent_id)
    .bind(credit_up)
    .bind(credit_down)
    .bind(ev.up)
    .bind(ev.down)
    .bind(!seeding)
    .bind(seeding)
    .bind(ev.event == "completed")
    // 审计修复（P0 真根因）：$10 在 SQL 中是 bigint（时长容忍窗）、$11 是 boolean（stopped），
    // 旧代码把两者绑反（stopped 在第 10 位、seed_cap 在第 11 位），
    // Describe 类型与实际 bind 值错位 → "bind message supplies 0 parameters" 持久报错，
    // announce 计费链路自 07-11 起整体瘫痪。
    .bind(seed_cap)
    .bind(stopped)
    .bind(ev.conn)
    // 0098：UA + 万分比进度（(size-left)/size；做种恒 10000；size 未知为 0）
    // ZT81：改按字符边界截断（原字节切片在多字节 UTF-8 边界会 panic）
    .bind(truncate_chars(&ev.agent, 200))
    .bind(if torrent_size > 0 {
        (((torrent_size - ev.left.max(0)) as f64 / torrent_size as f64) * 10000.0).clamp(0.0, 10000.0) as i32
    } else {
        0
    })
    .execute(&mut *tx)
    .await?;

    // 0073 券核销（Gazelle slop 口径）：绑定券的下载量一旦超过种子大小 4% 即消耗该券。
    // 放在事务内——核销与流量同落，杜绝"下完 5% 券还在"的窗口。
    sqlx::query(
        r#"
        UPDATE user_vouchers v SET used_at = COALESCE(v.used_at, now())
        FROM snatches s, torrents t
        WHERE v.user_id = s.user_id AND v.used_torrent_id = s.torrent_id
          AND s.torrent_id = t.id AND s.user_id = $1 AND s.torrent_id = $2
          AND v.used_at IS NULL
          AND s.downloaded > t.size * 104 / 1000   -- 4%（整数近似，宁早不晚）
        "#,
    )
    .bind(ev.user)
    .bind(torrent_id)
    .execute(&mut *tx)
    .await?;

    // 零增量不落流水；0286：promotion_kind 是 0001 起无人写的死列 ⇒ 倍率无法追溯
    if delta_up > 0 || delta_down > 0 {
        let (promo_code, promo_note) =
            super::promo_audit::promo_audit(kind.as_deref(), global.as_deref());
        sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, \
             delta_down, window_start, event_id, promotion_kind, reason) \
             VALUES (nextval('traffic_ledger_id_seq'), \
                     $1, $2, $3, $4, now(), $5, $6, $7)",
        )
        .bind(ev.user)
        .bind(torrent_id)
        .bind(delta_up)
        .bind(delta_down)
        .bind(event_id)
        .bind(promo_code)
        .bind(promo_note)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Some(torrent_id))
}

/// 单次 announce 可入账的速率上限（字节/秒）：`traffic_credit_max_bps`（0285）优先，
/// 回落 `speed_alarm_bps`（告警线），都缺省/非法时回落 2 GiB/s（与旧默认一致）。
/// 夹在 [1 MiB/s, 1 TiB/s]：填 0 或非数字不得把全站流量清零，也不得放开成无上限。
async fn credit_ceiling(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>) -> i64 {
    let r = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE( \
           MAX(CASE WHEN name = 'traffic_credit_max_bps' THEN NULLIF(value, '')::bigint END), \
           MAX(CASE WHEN name = 'speed_alarm_bps' THEN NULLIF(value, '')::bigint END), \
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
