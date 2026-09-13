//! 定时与消费任务。

use chrono::Datelike;
use sqlx::PgPool;

/// 存量种子 pieces_hash 回填（0069）：扫描 pieces_hash IS NULL 的种子，读 raw 计算 SHA1(info.pieces)。
/// 每轮 ≤50 条（避免一次长事务拖垮 worker），全部处理完后自然空转。worker 内置极简
/// bencode 解析（只依赖「定位 info 字典 → 取 pieces 字节串」，不重复造完整编解码）。
pub async fn backfill_pieces_hash(db: &PgPool) -> anyhow::Result<u64> {
    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT t.id, f.raw FROM torrents t \
         JOIN torrent_files f ON f.torrent_id = t.id \
         WHERE t.pieces_hash IS NULL LIMIT 50",
    )
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for (id, raw) in rows {
        if let Some(hash) = extract_pieces_hash(&raw) {
            sqlx::query(
                "UPDATE torrents SET pieces_hash = $1 WHERE id = $2 AND pieces_hash IS NULL",
            )
            .bind(hash)
            .bind(id)
            .execute(db)
            .await?;
            n += 1;
        } else {
            // 缺 pieces 字段的畸形种：写空串占位，避免每轮重复扫描
            sqlx::query(
                "UPDATE torrents SET pieces_hash = '' WHERE id = $1 AND pieces_hash IS NULL",
            )
            .bind(id)
            .execute(db)
            .await?;
        }
    }
    Ok(n)
}

/// 极简 bencode 遍历：返回 SHA1(info.pieces 原始字节) 的 hex。与 api 侧 bencode.rs 口径一致。
fn extract_pieces_hash(raw: &[u8]) -> Option<String> {
    use sha1::{Digest, Sha1};
    fn parse(buf: &[u8], pos: usize) -> Option<(BVal, usize)> {
        let rest = buf.get(pos..)?;
        match rest.first()? {
            b'i' => {
                let end = rest.iter().position(|&b| b == b'e')?;
                let n: i64 = std::str::from_utf8(&rest[1..end]).ok()?.parse().ok()?;
                Some((BVal::Int(n), pos + end + 1))
            }
            b'l' => {
                let mut p = pos + 1;
                let mut items = Vec::new();
                while *buf.get(p)? != b'e' {
                    let (v, np) = parse(buf, p)?;
                    items.push(v);
                    p = np;
                }
                Some((BVal::List(items), p + 1))
            }
            b'd' => {
                let mut p = pos + 1;
                let mut pairs = Vec::new();
                while *buf.get(p)? != b'e' {
                    let (k, np) = parse(buf, p)?;
                    let BVal::Bytes(kb) = k else { return None };
                    let (v, np2) = parse(buf, np)?;
                    pairs.push((kb, v));
                    p = np2;
                }
                Some((BVal::Dict(pairs), p + 1))
            }
            b if b.is_ascii_digit() => {
                let colon = rest.iter().position(|&c| c == b':')?;
                let len: usize = std::str::from_utf8(&rest[..colon]).ok()?.parse().ok()?;
                let start = pos + colon + 1;
                let end = start + len;
                if end > buf.len() {
                    return None;
                }
                Some((BVal::Bytes(buf[start..end].to_vec()), end))
            }
            _ => None,
        }
    }
    #[allow(dead_code)]
    enum BVal {
        Int(i64),
        Bytes(Vec<u8>),
        List(Vec<BVal>),
        Dict(Vec<(Vec<u8>, BVal)>),
    }
    let (root, _) = parse(raw, 0)?;
    let BVal::Dict(pairs) = root else { return None };
    let info = pairs.into_iter().find(|(k, _)| k == b"info")?.1;
    let BVal::Dict(info_pairs) = info else {
        return None;
    };
    let pieces = info_pairs.into_iter().find(|(k, _)| k == b"pieces")?.1;
    let BVal::Bytes(pieces) = pieces else {
        return None;
    };
    if pieces.is_empty() {
        return None;
    }
    let mut h = Sha1::new();
    h.update(&pieces);
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// 消费 agent_rules 命中事件（tracker emit_agent_block 投递）→ cheat_events 落库。
/// 同 (user_id, agent, reason) 累加 hits；首次命中投一条管理组信箱（staffmessages）。
pub async fn consume_agent_blocks(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    use redis::AsyncCommands;
    // 显式 turbofish：edition 2024 下 redis.get 的 never-type fallback 会拒绝 !: FromRedisValue
    let last_id: Option<String> = redis
        .get::<_, Option<String>>("flux:agentblock:cursor")
        .await
        .unwrap_or(None);
    let from = last_id.clone().unwrap_or_else(|| "-".to_string());
    let reply = match redis
        .xrange::<_, _, _, redis::streams::StreamRangeReply>("flux:agent_block", &from, "+")
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, "xrange flux:agent_block 失败");
            return Err(e.into());
        }
    };
    #[derive(serde::Deserialize)]
    struct AgentBlockEvent {
        user: i64,
        #[serde(default)]
        agent: String,
        #[serde(default)]
        ip: String,
        #[serde(default)]
        reason: String,
    }
    let mut applied = 0u64;
    let mut last_seen_id: Option<String> = None;
    for entry in reply.ids {
        let id = entry.id;
        if Some(&id) == last_id.as_ref() {
            last_seen_id = Some(id);
            continue;
        }
        let Some(payload) = entry
            .map
            .get("payload")
            .and_then(|v| redis::from_redis_value::<String>(v).ok())
        else {
            last_seen_id = Some(id);
            continue;
        };
        let Ok(ev) = serde_json::from_str::<AgentBlockEvent>(&payload) else {
            tracing::warn!(%id, "agent_block 事件解析失败，跳过");
            last_seen_id = Some(id);
            continue;
        };
        let agent = if ev.agent.len() > 128 {
            ev.agent[..128].to_string()
        } else {
            ev.agent
        };
        let reason = if ev.reason.len() > 200 {
            ev.reason[..200].to_string()
        } else {
            ev.reason
        };
        // 首次命中判定先于写入（tracker 侧已 1h 去重，这里的额外查询可忽略不计）
        let existed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cheat_events WHERE user_id = $1 AND agent = $2 AND reason = $3)",
        )
        .bind(ev.user)
        .bind(&agent)
        .bind(&reason)
        .fetch_one(db)
        .await
        .unwrap_or(true);
        let res = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) VALUES ($1, $2, $3, $4) \
             ON CONFLICT (user_id, agent, reason) DO UPDATE SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(ev.user)
        .bind(&agent)
        .bind(&ev.ip)
        .bind(&reason)
        .execute(db)
        .await;
        match res {
            Ok(_) => {
                applied += 1;
                last_seen_id = Some(id);
                if !existed {
                    // 首次命中 → 管理组信箱自动告警（staffmessages，permission=cheater 分流）
                    let body = format!(
                        "用户 #{} 的客户端命中黑白名单规则，tracker 已拒绝其 announce。\n客户端：{}\nIP：{}\n原因：{}\n（本条为系统自动告警，累计情况见后台「作弊探测」）",
                        ev.user,
                        if agent.is_empty() { "(空)" } else { &agent },
                        if ev.ip.is_empty() { "(未知)" } else { &ev.ip },
                        reason,
                    );
                    let _: Result<_, _> = sqlx::query(
                        "INSERT INTO staffmessages (user_id, subject, body, permission) \
                         VALUES ($1, '客户端黑白名单自动告警', $2, 'cheater')",
                    )
                    .bind(ev.user)
                    .bind(body)
                    .execute(db)
                    .await;
                }
            }
            Err(e) => {
                tracing::error!(%id, ?e, "cheat_events 写入失败，游标暂停等待重试");
                break;
            }
        }
    }
    if let Some(id) = last_seen_id {
        let cur: Result<(), redis::RedisError> = redis.set("flux:agentblock:cursor", &id).await;
        if let Err(e) = cur {
            tracing::error!(?e, "agentblock 游标写入失败（下轮可能重复计数）");
            return Err(e.into());
        }
        let _: () = redis
            .xtrim(
                "flux:agent_block",
                redis::streams::StreamMaxlen::Approx(5000),
            )
            .await
            .unwrap_or(());
    }
    Ok(applied)
}

/// 促销到期回收（M06：到期自动回收，无残留）。
pub async fn expire_promotions(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query("DELETE FROM promotions WHERE ends_at <= now()")
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}

/// 魔法池双免联动（原实现为死功能：promo_started 全库无写入点）。
/// 站点时区 UTC+8 每月 1-3 号检查上月是否达标（donated_total >= goal），
/// 达标则开全站 scope 双免促销（x2free，3 天），幂等靠 promo_started 标记。
pub async fn magic_pool_promo(db: &PgPool) -> anyhow::Result<u64> {
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    if !(1..=3).contains(&now_site.day()) {
        return Ok(0);
    }
    let res = sqlx::query(
        r#"
        WITH prev AS (
            SELECT to_char(date_trunc('month', $1::date) - interval '1 month', 'YYYY-MM') AS month
        ),
        pool AS (
            SELECT mp.month, mp.promo_started
            FROM magic_pool mp, prev
            WHERE mp.month = prev.month AND mp.donated_total >= mp.goal AND NOT mp.promo_started
        ),
        started AS (
            UPDATE magic_pool mp SET promo_started = TRUE
            FROM pool WHERE mp.month = pool.month
            RETURNING mp.month
        )
        INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source)
        SELECT 'global', NULL, 'x2free', now(), now() + interval '3 days', 'magic_pool'
        FROM started
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(now_site)
    .execute(db)
    .await?;
    let n = res.rows_affected();
    if n > 0 {
        tracing::info!(n, "magic_pool promo started (x2free, 3d)");
    }
    Ok(n)
}

/// 保种区移出（M19 旧站口径：做种 > 7 移出，免费延续 3 天）。
pub async fn preserve_exit(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH exited AS (
            UPDATE seed_preserve sp SET exited_at = now(), exit_reason = 'seeders_gt_7'
            FROM torrents t
            WHERE t.id = sp.torrent_id AND t.seeders > 7 AND sp.exited_at IS NULL
            RETURNING sp.torrent_id
        )
        INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source)
        SELECT 'torrent', torrent_id, 'free', now(), now() + interval '3 days', 'preserve_grace'
        FROM exited
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 做种收益小时结算（M11：基础火花 + 加成；捐赠者 2x）。
/// 每用户每小时一条流水，幂等键 = seeding:{user}:{yyyymmddhh}。
pub async fn seeding_reward(db: &PgPool, base: i64) -> anyhow::Result<u64> {
    let hour = chrono::Utc::now().format("%Y%m%d%H").to_string();
    let res = sqlx::query(
        r#"
        WITH earners AS (
            SELECT u.id, u.donor,
                   count(*) AS seeding_count, COALESCE(sum(t.size),0) AS seeding_size
            FROM users u
            JOIN snatches s ON s.user_id = u.id AND s.seeding
            JOIN torrents t ON t.id = s.torrent_id
            WHERE u.status < 2
            GROUP BY u.id, u.donor
        ),
        due AS (
            SELECT id,
                   ($1 + (seeding_count * 2 + seeding_size / 1099511627776))::bigint
                     * CASE WHEN donor THEN 2 ELSE 1 END AS amount,
                   'seeding:' || id || ':' || $2 AS idem
            FROM earners
        )
        INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
        SELECT nextval('spark_ledger_id_seq'), id, amount, 'seeding_reward', idem
        FROM due
        WHERE NOT EXISTS (
            SELECT 1 FROM spark_ledger l WHERE l.idempotency_key = due.idem
        )
        "#,
    )
    .bind(base)
    .bind(&hour)
    .execute(db)
    .await?;
    // 刷新余额快照（权威在流水，快照仅展示）
    sqlx::query(
        "UPDATE users SET spark_balance = COALESCE((             SELECT sum(amount) FROM spark_ledger WHERE user_id = users.id          ), 0)",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

#[derive(serde::Deserialize)]
struct AnnounceEvent {
    user: i64,
    hash: String,
    up: i64,
    down: i64,
    #[serde(default)]
    event: String,
    #[serde(default)]
    left: i64,
}

/// 消费 announce 事件流（§5.4 链路 ④-⑦）：Redis Stream → 计费流水 + snatches。
/// 事件为 JSON 文本，字段 user/hash/up/down/event/left。
pub async fn consume_announce(
    db: &PgPool,
    redis: &mut redis::aio::ConnectionManager,
) -> anyhow::Result<u64> {
    let redis_dead_letter = redis.clone();
    use redis::AsyncCommands;

    // 游标消费：从上次处理到的 ID 继续拉取（Redis 键持久化游标，重启不丢事件、不重复计费）
    let last_id: Option<String> = redis.get("flux:announce:cursor").await.unwrap_or(None);
    let from = last_id.clone().unwrap_or_else(|| "-".to_string());

    // 保种时长累计容忍窗 = 2 × announce_interval（与 tracker 下发口径一致，站点设定缺省 1800）。
    // 客户端按 interval 汇报，相邻两次做种 announce 的时间差即真实做种时长；
    // 超窗（离线/故障）不计，防挂机伪造做种时长。
    let announce_interval: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    let seed_cap = (announce_interval * 2).clamp(3600, 172_800);

    // XRANGE → StreamRangeReply（redis 0.27 类型映射；错误必须可见，不允许静默空消费）
    let reply = match redis
        .xrange::<_, _, _, redis::streams::StreamRangeReply>("flux:announce", &from, "+")
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(?e, "xrange flux:announce 失败");
            return Err(e.into());
        }
    };

    let mut applied = 0u64;
    let mut last_seen_id: Option<String> = None;
    for entry in reply.ids {
        let id = entry.id;
        // 已处理过的游标本条跳过
        if Some(&id) == last_id.as_ref() {
            last_seen_id = Some(id);
            continue;
        }
        let Some(payload) = entry
            .map
            .get("payload")
            .and_then(|v| redis::from_redis_value::<String>(v).ok())
        else {
            // 损坏事件进死信，不阻塞游标
            tracing::warn!(%id, "announce 事件损坏，进死信");
            let mut conn_dl = redis_dead_letter.clone();
            let _: Result<(), _> = redis::cmd("RPUSH")
                .arg("flux:announce:dlq")
                .arg(&id)
                .query_async(&mut conn_dl)
                .await;
            last_seen_id = Some(id);
            continue;
        };
        let ev = match serde_json::from_str::<AnnounceEvent>(&payload) {
            Ok(ev) => ev,
            Err(e) => {
                tracing::warn!(%id, ?e, "事件 JSON 解析失败，进死信");
                let mut conn_dl = redis_dead_letter.clone();
                let _: Result<(), _> = redis::cmd("RPUSH")
                    .arg("flux:announce:dlq")
                    .arg(&payload)
                    .query_async(&mut conn_dl)
                    .await;
                last_seen_id = Some(id);
                continue;
            }
        };
        match process_event(db, &ev, seed_cap).await {
            Ok(()) => {
                applied += 1;
                last_seen_id = Some(id);
            }
            Err(e) => {
                // 处理失败：不推进游标，下轮重试（避免丢失计费）
                tracing::error!(%id, ?e, "事件计费失败，游标暂停等待重试");
                break;
            }
        }
    }
    if let Some(id) = last_seen_id {
        let cur: Result<(), redis::RedisError> = redis.set("flux:announce:cursor", &id).await;
        if let Err(e) = cur {
            // 游标写入失败必须显式报错：静默失败会导致下轮重复计费
            tracing::error!(?e, "游标写入失败（下轮可能重复计费，需人工核对）");
            return Err(e.into());
        }
        let _: () = redis
            .xtrim("flux:announce", redis::streams::StreamMaxlen::Approx(10000))
            .await
            .unwrap_or(());
    }
    // 刷新用户上/下载量快照（权威在 traffic_ledger 流水，§6.2 快照仅展示）
    sqlx::query(
        "UPDATE users SET             uploaded = COALESCE((SELECT sum(delta_up) FROM traffic_ledger WHERE user_id = users.id), 0),             downloaded = COALESCE((SELECT sum(delta_down) FROM traffic_ledger WHERE user_id = users.id), 0)",
    )
    .execute(db)
    .await?;
    // 回填种子做种/下载计数（详情页与保种规则数据源）
    sqlx::query(
        "UPDATE torrents t SET             seeders = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.seeding), 0),             leechers = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.leeching), 0),             times_completed = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL), 0)",
    )
    .execute(db)
    .await?;
    Ok(applied)
}

/// 单事件计费（促销裁决 + snatch upsert + 流水 + 保种时长累计）
/// seed_cap：做种时长单次累计容忍窗（秒）= 2 × announce_interval，由 consume_announce 按站点设定算出
async fn process_event(db: &PgPool, ev: &AnnounceEvent, seed_cap: i64) -> anyhow::Result<()> {
    // 未知种子的查询失败必须显式报错（重试），不能静默丢弃计费
    let torrent_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM torrents WHERE info_hash = $1")
            .bind(&ev.hash)
            .fetch_optional(db)
            .await?;
    let Some(torrent_id) = torrent_id else {
        return Ok(()); // 种子确实不存在：跳过
    };

    // 促销快照裁决（§5.4-⑦）——与 API 展示口径一致：同种子多条专属促销取最强档
    // （修复前 ORDER BY id DESC 只认最新一条：先挂 free 后挂 half 时计费取 half、展示取 free）
    let kind: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions WHERE torrent_id = $1 AND starts_at <= now() AND ends_at > now() \
         ORDER BY CASE kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 \
                                  WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, id DESC \
         LIMIT 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await?;
    let global: Option<String> = sqlx::query_scalar(
        "SELECT kind::text FROM promotions p \
         WHERE p.torrent_id IS NULL AND p.starts_at <= now() AND p.ends_at > now() \
           AND (p.scope = 'global' \
                OR (p.scope = 'official' AND EXISTS (SELECT 1 FROM torrents t WHERE t.id = $1 AND t.official_tag)) \
                OR (p.scope = 'non_official' AND EXISTS (SELECT 1 FROM torrents t WHERE t.id = $1 AND NOT t.official_tag)) \
                OR (p.scope = 'category' AND EXISTS (SELECT 1 FROM torrents t WHERE t.id = $1 AND t.category_id = p.category_id))) \
         ORDER BY CASE kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 \
                                  WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC \
         LIMIT 1",
    )
    .fetch_optional(db)
    .await?;
    let (up_mult, down_mult) = billing_multipliers(kind.as_deref(), global.as_deref());

    // BEP3：ev.up/down 是客户端累计总量 —— 先取出上次上报值换算增量（P0 修复）
    let mut tx = db.begin().await?;
    let last: Option<(i64, i64)> = sqlx::query_as(
        "SELECT last_up, last_down FROM snatches WHERE user_id = $1 AND torrent_id = $2 FOR UPDATE",
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

    let seeding = ev.left == 0;
    // stopped = 客户端退出：与 tracker 侧 remove(peer) 对齐，DB 也不应继续标记在做种/下载
    let stopped = ev.event == "stopped";
    sqlx::query(
        r#"
        INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, last_up, last_down, leeching, seeding, completed_at, last_seen_at)
        VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $11 THEN FALSE ELSE $7 END, CASE WHEN $11 THEN FALSE ELSE $8 END, CASE WHEN $9 THEN now() ELSE NULL END, now())
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
    .bind(stopped)
    .bind(seed_cap)
    .execute(&mut *tx)
    .await?;
    // 仅在有实际增量时落流水（避免零增量噪声）
    if delta_up > 0 || delta_down > 0 {
        sqlx::query(
            "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start)          VALUES (nextval('traffic_ledger_id_seq'), $1, $2, $3, $4, now())",
        )
        .bind(ev.user)
        .bind(torrent_id)
        .bind(delta_up)
        .bind(delta_down)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// 做种里程碑采集（M28 插件数据源）：把达到档位的事件落表，api 侧插件按需消费。
/// 幂等：UNIQUE(user_id, torrent_id, hours) + ON CONFLICT DO NOTHING。
async fn collect_milestones(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        INSERT INTO seed_milestones (id, user_id, torrent_id, hours)
        SELECT nextval('seed_milestones_id_seq'), user_id, torrent_id, h.hours
        FROM snatches s
        CROSS JOIN (VALUES (24), (168), (720), (2160)) AS h(hours)
        WHERE s.seeding
          AND EXTRACT(EPOCH FROM (now() - s.completed_at))::bigint / 3600 >= h.hours
          AND s.completed_at IS NOT NULL
        ON CONFLICT (user_id, torrent_id, hours) DO NOTHING
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "seed milestones collected");
    }
    Ok(res.rows_affected())
}

/// H&R 追责（M05 补齐）：为「完成下载」建立策略快照（时点正确），到期结算违规。
/// 策略口径（§5.4）：hr_policy JSONB {"days": N, "seed_hours": H} —— 完成后 N 天内需累计做种 H 小时。
/// B-01：完成时刻正处免费（free/x2free，含全局站免）窗口的种子豁免 H&R —— 行业惯例。
async fn hr_enforce(db: &PgPool) -> anyhow::Result<()> {
    // 1) 为新完成的下载建快照（幂等）；免费窗口内完成的不建快照（豁免）
    sqlx::query(
        r#"
        INSERT INTO hr_snapshots (user_id, torrent_id, required_seconds, deadline)
        SELECT s.user_id, s.torrent_id,
               COALESCE((t.hr_policy->>'seed_hours')::int, 48) * 3600,
               s.completed_at + make_interval(days => COALESCE((t.hr_policy->>'days')::int, 14))
        FROM snatches s
        JOIN torrents t ON t.id = s.torrent_id
        WHERE s.completed_at IS NOT NULL
          AND NOT EXISTS (SELECT 1 FROM hr_snapshots h WHERE h.user_id = s.user_id AND h.torrent_id = s.torrent_id)
          AND COALESCE(t.hr_policy->>'enabled', 'true')::boolean
          AND NOT EXISTS (
              SELECT 1 FROM promotions p
              WHERE (p.torrent_id = s.torrent_id
                     OR (p.torrent_id IS NULL AND (
                         p.scope = 'global'
                         OR (p.scope = 'official' AND t.official_tag)
                         OR (p.scope = 'non_official' AND NOT t.official_tag)
                         OR (p.scope = 'category' AND t.category_id = p.category_id))))
                AND p.starts_at <= s.completed_at AND p.ends_at > s.completed_at
                AND p.kind IN ('free', 'x2free')
          )
          -- 保种员 / VIP 持 hr.exempt 权限 → 免除 H&R，不建快照
          AND NOT user_can(s.user_id, 'hr.exempt')
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(db)
    .await?;

    // 2) 刷新累计做种秒数（快照口径：snatches.seeded_seconds）
    sqlx::query(
        "UPDATE hr_snapshots h SET seeded_seconds = s.seeded_seconds, updated_at = now()          FROM snatches s          WHERE s.user_id = h.user_id AND s.torrent_id = h.torrent_id AND h.status = 'open'",
    )
    .execute(db)
    .await?;

    // 3) 达标即 satisfied
    sqlx::query(
        "UPDATE hr_snapshots SET status = 'satisfied', updated_at = now()          WHERE status = 'open' AND seeded_seconds >= required_seconds",
    )
    .execute(db)
    .await?;

    // 4) 过期未达标 → violated + 落违规表（追责依据）
    let violated = sqlx::query(
        r#"
        WITH dead AS (
            UPDATE hr_snapshots SET status = 'violated', updated_at = now()
            WHERE status = 'open' AND deadline < now()
            RETURNING user_id, torrent_id, seeded_seconds, required_seconds
        )
        INSERT INTO hr_violations (user_id, torrent_id, seeded_seconds, required_seconds)
        SELECT user_id, torrent_id, seeded_seconds, required_seconds FROM dead
        ON CONFLICT DO NOTHING
        "#,
    )
    .execute(db)
    .await?;
    if violated.rows_affected() > 0 {
        tracing::warn!(n = violated.rows_affected(), "H&R violations detected");
    }

    // 5) hr_flag 刷新（0029 一次性迁移的运行时延续）：完成已超 14 天且做种时长 < 120h。
    //    此前该标记只在迁移里置过一次，运行时无人刷新 —— /me/hr（community_http）口径失真。
    sqlx::query(
        "UPDATE snatches SET hr_flag = TRUE \
         WHERE completed_at IS NOT NULL AND seeded_seconds < 432000 \
           AND completed_at < now() - interval '14 days' AND NOT hr_flag",
    )
    .execute(db)
    .await?;
    Ok(())
}

/// 等级自动升降（class_rules）：达标即升（逐级检查），不达标且 demotable 则降至仍满足的最高级。
async fn class_auto_adjust(db: &PgPool) -> anyhow::Result<()> {
    let promoted = sqlx::query(
        r#"
        WITH stats AS (
            SELECT u.id, u.class_id, u.uploaded,
                   (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.completed_at IS NOT NULL) AS dl,
                   (SELECT COALESCE(sum(s.seeded_seconds),0)/3600 FROM snatches s WHERE s.user_id = u.id) AS sh,
                   EXTRACT(DAY FROM now() - u.created_at)::bigint AS age
            FROM users u WHERE u.class_id < 90
        ),
        target AS (
            SELECT s.id, max(r.class_id) AS new_class
            FROM stats s JOIN class_rules r ON
                s.uploaded >= r.min_uploaded AND s.dl >= r.min_download_count AND
                s.sh >= r.min_seed_hours AND s.age >= r.min_account_age_days
            GROUP BY s.id
        )
        UPDATE users u SET class_id = t.new_class
        FROM target t WHERE u.id = t.id AND t.new_class > u.class_id
        "#,
    )
    .execute(db)
    .await?;
    let demoted = sqlx::query(
        r#"
        WITH stats AS (
            SELECT u.id, u.class_id, u.uploaded,
                   (SELECT count(*) FROM snatches s WHERE s.user_id = u.id AND s.completed_at IS NOT NULL) AS dl,
                   (SELECT COALESCE(sum(s.seeded_seconds),0)/3600 FROM snatches s WHERE s.user_id = u.id) AS sh,
                   EXTRACT(DAY FROM now() - u.created_at)::bigint AS age
            FROM users u JOIN class_rules cr ON cr.class_id = u.class_id
            WHERE u.class_id < 90 AND cr.demotable
        ),
        target AS (
            -- 仍满足的最高级；一条都不满足 → 1（保底不降为 0）
            SELECT s.id, COALESCE(max(r.class_id), 1) AS new_class
            FROM stats s JOIN class_rules r ON
                s.uploaded >= r.min_uploaded AND s.dl >= r.min_download_count AND
                s.sh >= r.min_seed_hours AND s.age >= r.min_account_age_days
            GROUP BY s.id
        )
        UPDATE users u SET class_id = t.new_class
        FROM target t WHERE u.id = t.id AND t.new_class < u.class_id
        "#,
    )
    .execute(db)
    .await?;
    if promoted.rows_affected() > 0 {
        tracing::info!(n = promoted.rows_affected(), "users promoted");
    }
    if demoted.rows_affected() > 0 {
        tracing::info!(n = demoted.rows_affected(), "users demoted");
    }
    Ok(())
}

/// 僵尸做种/下载标记清理：tracker peer 表 90s 超时即除名，但 DB 侧 snatches.seeding/leeching
/// 原本只在下一次 announce 时被覆盖 —— 客户端崩溃/卸载（无 stopped 事件）的行会永久保持
/// seeding=true，导致 seeding_reward 空转发钱（live 证据：27 行 last_seen 2 天前仍在领收益）
/// 与 torrents.seeders 虚高。阈值 = max(2h, 2×announce_interval)，远大于正常重汇报抖动。
async fn sweep_stale_peers(db: &PgPool) -> anyhow::Result<u64> {
    let interval: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    let threshold_secs = (interval * 2).max(7200);
    let res = sqlx::query(
        "UPDATE snatches SET seeding = FALSE, leeching = FALSE \
         WHERE (seeding OR leeching) AND last_seen_at < now() - ($1::bigint * interval '1 second')",
    )
    .bind(threshold_secs)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 登录事件留存清理：IP 属个人信息，90 天后删除（每小时一次，幂等）。
async fn purge_old_login_events(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query("DELETE FROM login_events WHERE created_at < now() - interval '90 days'")
        .execute(db)
        .await?;
    Ok(res.rows_affected())
}

/// 主循环：定时任务调度。
pub async fn run_all(db: PgPool, mut redis: redis::aio::ConnectionManager) -> anyhow::Result<()> {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut hour_tick = tokio::time::interval(std::time::Duration::from_secs(3600));
    let mut first_hour = true;
    let mut last_bank_day: Option<chrono::NaiveDate> = None;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                if let Err(e) = expire_promotions(&db).await { tracing::error!(?e, "expire_promotions"); }
                if let Err(e) = magic_pool_promo(&db).await { tracing::error!(?e, "magic_pool_promo"); }
                if let Err(e) = preserve_exit(&db).await { tracing::error!(?e, "preserve_exit"); }
                if let Err(e) = consume_announce(&db, &mut redis).await { tracing::error!(?e, "consume_announce"); }
                if let Err(e) = consume_agent_blocks(&db, &mut redis).await { tracing::error!(?e, "consume_agent_blocks"); }
                if let Err(e) = backfill_pieces_hash(&db).await { tracing::error!(?e, "backfill_pieces_hash"); }
                if let Err(e) = sweep_stale_peers(&db).await { tracing::error!(?e, "sweep_stale_peers"); }
                if let Err(e) = collect_milestones(&db).await { tracing::error!(?e, "collect_milestones"); }
                if let Err(e) = hr_enforce(&db).await { tracing::error!(?e, "hr_enforce"); }
                if let Err(e) = class_auto_adjust(&db).await { tracing::error!(?e, "class_auto_adjust"); }
                if let Err(e) = crate::task_jobs::task_settle(&db).await { tracing::error!(?e, "task_settle"); }
                // 银行结算：站点时区 UTC+8 自然日切换后跑一次；分钟级检查保证 worker 重启/宕机跨日也能补跑
                let site_day = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
                if last_bank_day.is_none() {
                    last_bank_day = Some(init_bank_day(&db).await);
                }
                if last_bank_day != Some(site_day) {
                    tracing::info!(?site_day, "bank_daily start");
                    crate::bank_jobs::bank_daily(&db).await;
                    last_bank_day = Some(site_day);
                }
            }
            _ = hour_tick.tick() => {
                if first_hour { first_hour = false; continue; }
                if let Err(e) = seeding_reward(&db, 10).await { tracing::error!(?e, "seeding_reward"); }
                if let Err(e) = purge_old_login_events(&db).await { tracing::error!(?e, "purge_old_login_events"); }
            }
        }
    }
}

/// 启动基线：当日（站点时区）已由上一进程结算过则不重跑，取健康游标最近记录日期。
async fn init_bank_day(db: &PgPool) -> chrono::NaiveDate {
    let site_today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let last_run: Option<chrono::NaiveDate> =
        sqlx::query_scalar("SELECT max(run_date) FROM bank_settle_runs")
            .fetch_one(db)
            .await
            .ok()
            .flatten();
    match last_run {
        // 今日已结 → 以今日为基线（当日不重跑）；更早/无记录 → 昨日（跨日即触发）
        Some(d) if d >= site_today => d,
        _ => site_today - chrono::Duration::days(1),
    }
}

/// 促销计费倍率（M06 倍率表，与 api domain::PromotionKind::multipliers 同口径）
fn billing_multipliers(torrent_kind: Option<&str>, global_kind: Option<&str>) -> (f64, f64) {
    let strength = |k: &str| -> u8 {
        match k {
            "p30" => 1,
            "half" => 2,
            "free" => 3,
            "x2" => 4,
            "x2half" => 5,
            "x2free" => 6,
            _ => 0,
        }
    };
    let table = |k: Option<&str>| -> (f64, f64) {
        match k {
            Some("free") => (1.0, 0.0),
            Some("x2") => (2.0, 1.0),
            Some("x2free") => (2.0, 0.0),
            Some("half") => (1.0, 0.5),
            Some("x2half") => (2.0, 0.5),
            Some("p30") => (1.0, 0.3),
            _ => (1.0, 1.0),
        }
    };
    let winner = match (torrent_kind, global_kind) {
        (Some(t), Some(g)) => Some(if strength(t) >= strength(g) { t } else { g }),
        (t, g) => t.or(g),
    };
    table(winner)
}

#[cfg(test)]
mod tests {
    use super::billing_multipliers;

    #[test]
    fn free_zeroes_download() {
        assert_eq!(billing_multipliers(Some("free"), None), (1.0, 0.0));
    }

    #[test]
    fn stronger_promotion_wins() {
        assert_eq!(billing_multipliers(Some("free"), Some("x2")), (2.0, 1.0));
        assert_eq!(
            billing_multipliers(Some("x2free"), Some("free")),
            (2.0, 0.0)
        );
    }

    #[test]
    fn none_is_normal() {
        assert_eq!(billing_multipliers(None, None), (1.0, 1.0));
    }
}
