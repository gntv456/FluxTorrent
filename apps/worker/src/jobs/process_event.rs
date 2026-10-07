//! announce 事件计费处理。
//! 从 jobs.rs 按域拆出。

use super::announce_main::AnnounceEvent;
use super::billing_mults::resolve_billing_mults;
use super::ledger_guard::ledger_guard;
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

    let (up_mult, down_mult, kind, global) =
        resolve_billing_mults(db, ev, torrent_id).await?;

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

    let (
        anchor_up,
        anchor_down,
        credit_up,
        credit_down,
        had_baseline,
        held_up,
        held_down,
    ) = ledger_guard(&mut tx, ev, torrent_id, torrent_size, seed_cap).await?;
    let delta_up = (credit_up as f64 * up_mult) as i64;
    let delta_down = (credit_down as f64 * down_mult) as i64;

    // 超上限留痕：agent 沿用 cheat_audit 的 torrent:{id} 约定、speed: 前缀区分来源，
    // reason 带被扣量与证据，供管理组复核后用补量接口发还。
    if had_baseline && (held_up > 0 || held_down > 0) {
        let _ = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, agent, reason) DO UPDATE \
               SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(ev.user)
        .bind(format!("speed:{torrent_id}"))
        .bind(&ev.ip)
        // 二轮审计：reason 是去重键的一部分，内嵌每次都变的数值会让
        // ON CONFLICT 恒不命中 → hits 聚合失效、行无限膨胀，管理端前 200
        // 条可被"每次超一点"的攻击冲掉（告警稀释）。数值挪出键：
        // reason 固定文案聚合，明细走 tracing（有 request 上下文可查）。
        .bind("over_ceiling（增量超物理速率上限，超出部分未入账）")
        .execute(&mut *tx)
        .await;
        tracing::warn!(
            user = ev.user,
            torrent = torrent_id,
            held_up,
            held_down,
            "增量超物理速率上限，超出部分未入账"
        );
    }

    // P0-2 幽灵做种三条件（保种组实测审计 2026-10-07）：left=0（数据完整）
    // AND port>0（客户端开了监听端口——纯 curl 伪造 announce 恒 port=0）
    // AND connectable!=0（回连不可达的挂种不算在种；None=本次未测，放行）。
    // 实测旧口径下「无端口、无文件、无监听」的裸 HTTP GET 即可令 seeding=true、
    // seeders+1、seeded_seconds 持续累计，做种收益/保种区/复活任务全链路可刷。
    let seeding = ev.left == 0 && ev.port > 0 && ev.conn != Some(0);
    // P0-2C completed 下载侧证据：累计入账下载量为 0 的 "completed" 是伪造
    // （连一个字节都没下载过就宣布完成）。H&R buffer 用 10% 口径，这里只做
    // 非零下限——宽松但足以拦「从未下载、伪造事件直接挂种」的路径。
    let completed = ev.event == "completed" && ev.down > 0;
    // stopped = 客户端退出：与 tracker 侧 remove(peer) 对齐，DB 也不应继续标记在做种/下载
    let stopped = ev.event == "stopped";
    sqlx::query(
        r#"
        INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, last_up, last_down, leeching, seeding, completed_at, last_seen_at, connectable, agent, progress, last_port)
        VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $11 THEN FALSE ELSE $7 END, CASE WHEN $11 THEN FALSE ELSE $8 END, CASE WHEN $9 THEN now() ELSE NULL END, now(), COALESCE($12, 1), $13, $14, $15)
        ON CONFLICT (user_id, torrent_id) DO UPDATE SET
          uploaded = snatches.uploaded + EXCLUDED.uploaded,
          downloaded = snatches.downloaded + EXCLUDED.downloaded,
          -- P0-1 基线单向（2026-10-07 审计）：last_up/last_down 是增量换算的锚点，
          -- 必须只进不退——EXCLUDED 已是 Rust 侧守卫后的锚点（大幅回退沿用旧值），
          -- GREATEST 再兜一层，防止任何路径把锚点拉低后重吃增量。
          last_up = GREATEST(snatches.last_up, EXCLUDED.last_up),
          last_down = GREATEST(snatches.last_down, EXCLUDED.last_down),
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
          -- P2（2026-10-07 审计）：UDP 事件 agent 恒空，不能用它冲掉 HTTP 侧
          -- 记录的真实 UA（取证口径以 HTTP announce 为准）
          agent = COALESCE(NULLIF(EXCLUDED.agent, ''), snatches.agent),
          progress = EXCLUDED.progress,
          last_seen_at = now()
        "#,
    )
    .bind(ev.user)
    .bind(torrent_id)
    .bind(credit_up)
    .bind(credit_down)
    // P0-1：写库锚点用守卫后的 anchor（大幅回退时沿用旧基线），与
    // upsert 侧的 GREATEST 双保险——EXCLUDED.last_up 永远是「合法单调」的。
    .bind(anchor_up)
    .bind(anchor_down)    .bind(!seeding)
    .bind(seeding)
    .bind(completed)
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
    // P0-2：上报端口（幽灵做种判定基础；i32 与迁移 0298 列型一致）
    .bind(ev.port as i32)
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
