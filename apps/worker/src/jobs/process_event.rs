//! announce 事件计费处理。
//! 从 jobs.rs 按域拆出。

use super::announce_main::AnnounceEvent;
use super::billing::billing_multipliers;
use sqlx::PgPool;

/// 单事件计费（促销裁决 + snatch upsert + 流水 + 保种时长累计）
/// 返回 torrent_id 供快照点刷收集（种子不存在返回 None）。
/// seed_cap：做种时长单次累计容忍窗（秒）= 2 × announce_interval，由 consume_announce 按站点设定算出
pub(crate) async fn process_event(
    db: &PgPool,
    ev: &AnnounceEvent,
    seed_cap: i64,
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
    let last: Option<(i64, i64)> = sqlx::query_as(
                "SELECT last_up, \
         last_down FROM snatches WHERE user_id = $1 AND torrent_id = $2 FOR UPDATE",
    )
    .bind(ev.user)
    .bind(torrent_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (last_up, last_down) = last.unwrap_or((0, 0));
    // 计数器回绕/客户端重置时按 0 处理
    let raw_up = (ev.up - last_up).max(0);
    let raw_down = (ev.down - last_down).max(0);
    let delta_up = (raw_up as f64 * up_mult) as i64;
    let delta_down = (raw_down as f64 * down_mult) as i64;

    // 实时速度反作弊（NP announce 侧口径）：本次上报增量 ÷ 距上次上报间隔 得均速，
    // 超物理阈值（默认 2GB/s，site_settings.speed_alarm_bps 可调）→ 记 cheat_events。
    // 首次上报（无 last 行）算不出间隔，跳过；只记事件不打断计费（离线复核后处置）。
    if last.is_some() && (raw_up > 0 || raw_down > 0) {
        let interval: Option<i64> = sqlx::query_scalar(
            "SELECT EXTRACT(EPOCH FROM (now() - last_seen_at))::bigint \
             FROM snatches WHERE user_id = $1 AND torrent_id = $2",
        )
        .bind(ev.user)
        .bind(torrent_id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();
        if let Some(secs) = interval {
            if secs >= 30 {
                let bps = (raw_up.max(raw_down) as f64 / secs as f64) as i64;
                let threshold: i64 = sqlx::query_scalar(
                    "SELECT COALESCE((SELECT value FROM \
                     site_settings WHERE name = 'speed_alarm_bps')::bigint, \
                     2147483648)",
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(2_147_483_648);
                if bps > threshold {
                    // agent 字段沿用 cheat_audit 的 torrent:{id} 约定（speed: 前缀区分来源），reason 带证据
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
                        "speed_anomaly {bps}B/s over {secs}s up={raw_up} down={raw_down}"
                    ))
                    .execute(&mut *tx)
                    .await;
                    tracing::warn!(
                        user = ev.user,
                        torrent = torrent_id,
                        bps,
                        "speed anomaly recorded"
                    );
                }
            }
        }
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
    .bind(raw_up)
    .bind(raw_down)
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
    .bind(ev.agent[..ev.agent.len().min(200)].to_string())
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

    // 仅在有实际增量时落流水（避免零增量噪声）
    if delta_up > 0 || delta_down > 0 {
        sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, \
             delta_up, delta_down, window_start) VALUES \
             (nextval('traffic_ledger_id_seq'), $1, $2, $3, $4, now())",
        )
        .bind(ev.user)
        .bind(torrent_id)
        .bind(delta_up)
        .bind(delta_down)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(Some(torrent_id))
}
