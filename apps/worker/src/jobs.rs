//! 定时与消费任务。

use chrono::Datelike;
use sqlx::{PgPool, Row};

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

/// 做种收益小时结算（0074 改造：稀有度衰减 + 时长饱和，NP/U3D 口径融合）。
/// 每用户每小时一条流水，幂等键 = seeding:{user}:{yyyymmddhh}。
///
/// 公式（口径由 `mod tests` 内 mirror/user_hourly 单测锁定）：
///   每种子加成 = 规则分（濒危 2.0/高龄 1.5/老 1.0/大 0.75/中 0.5/日常 0.25——U3D 可读规则名）
///             × 稀有度因子 seeders^-0.35（Gazelle BP 口径：人多衰减，防大户垄断热门种）
///             × 时长饱和因子 1/(1+seedtime_h/2160)（90 天半衰）
///   每小时 = base + 400/π·atan(Σ加成 × 6/50)（NP arctan 软封顶：渐近 +200/h，底薪不进压缩曲线）。
/// 反假保种（0071）：回连不可达且从无上传的做种行不计。
pub async fn seeding_reward(db: &PgPool, base: i64) -> anyhow::Result<u64> {
    let hour = chrono::Utc::now().format("%Y%m%d%H").to_string();
    let res = sqlx::query(
        r#"
        WITH per_torrent AS (
            SELECT u.id AS user_id, u.donor, t.id AS torrent_id,
                   CASE
                     WHEN t.seeders <= 1 AND t.times_completed >= 3 THEN 2.0   -- 濒危保种（U3D Dying）
                     WHEN now() - t.created_at > interval '365 days' THEN 1.5   -- 高龄种（Legendary）
                     WHEN now() - t.created_at > interval '180 days' THEN 1.0   -- 老种（Old）
                     WHEN t.size >= 107374182400 THEN 0.75                      -- 大体积 ≥100GiB
                     WHEN t.size >= 26843545600 THEN 0.5                        -- 中体积 25-100GiB
                     ELSE 0.25                                                  -- 日常种
                   END AS rule_bonus,
                   GREATEST(t.seeders, 1)::numeric AS seeders_n,
                   GREATEST(s.seeded_seconds, 0)::numeric / 3600.0 AS seed_hours
            FROM users u
            JOIN snatches s ON s.user_id = u.id AND s.seeding
            JOIN torrents t ON t.id = s.torrent_id
              AND NOT (s.connectable = 0 AND s.uploaded = 0)
            WHERE u.status < 2
        ),
        per_user AS (
            SELECT user_id, donor,
                   sum( rule_bonus
                        / power(seeders_n, 0.35)
                        / (1 + seed_hours / 2160.0)
                      ) AS bonus_raw
            FROM per_torrent GROUP BY user_id, donor
        ),
        due AS (
            -- 底薪不进压缩曲线；加成部分 ×6/50 快速进入 atan 饱和区（渐近 +200/h）
            SELECT user_id AS id,
                   ($1::bigint + (400.0 / 3.14159265 * atan(bonus_raw * 6.0 / 50.0))::bigint)
                     * CASE WHEN donor THEN 2 ELSE 1 END AS amount,
                   'seeding:' || user_id || ':' || $2 AS idem
            FROM per_user
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
    // 0072：顺手物化做种体积（ptppUserInfo 的 seedingSize 字段来源；earners 已扫全量做种行）
    sqlx::query(
        "UPDATE users u SET seeding_size = COALESCE(s.sz, 0) \
         FROM (SELECT s2.user_id, sum(t2.size) AS sz FROM snatches s2 \
               JOIN torrents t2 ON t2.id = s2.torrent_id WHERE s2.seeding GROUP BY s2.user_id) s \
         WHERE u.id = s.user_id",
    )
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
    /// announce 来源 IP（0071 反作弊：账号 IP 跳变/多 IP 分析）
    #[serde(default)]
    ip: String,
    /// tracker 主动回连抽样结果（0071）：-1=未测（缺省/旧事件） 0=不可达 1=可达
    #[serde(default)]
    conn: Option<i16>,
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
    // P0-1 快照增量化：只刷本轮涉及的用户/种子（此前每轮对 users/torrents 全表重算，
    // 万级种子下每分钟两次全表聚合；纠偏由 run_all 的 reconcile_snapshots 周期兜底）
    let mut touched_users: std::collections::BTreeSet<i64> = Default::default();
    let mut touched_torrents: std::collections::BTreeSet<i64> = Default::default();
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
            Ok(Some(torrent_id)) => {
                applied += 1;
                last_seen_id = Some(id);
                touched_users.insert(ev.user);
                touched_torrents.insert(torrent_id);
                // connectable 抽样联动（0071 P1-9）：不可达 + 零上传 → 疑似假保种，记入作弊探测
                if ev.conn == Some(0) {
                    let _ = sqlx::query(
                        "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
                         VALUES ($1, 'connectable', $2, 'suspect_ghost_seed') \
                         ON CONFLICT (user_id, agent, reason) DO UPDATE \
                           SET hits = cheat_events.hits + 1, last_seen = now()",
                    )
                    .bind(ev.user)
                    .bind(&ev.ip)
                    .execute(db)
                    .await;
                }
            }
            Ok(None) => {
                // 种子不存在：跳过计费但推进游标
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
    // P0-1：快照点刷——仅本轮有事件的用户/种子（权威在流水，快照仅展示）
    let users: Vec<i64> = touched_users.into_iter().collect();
    if !users.is_empty() {
        sqlx::query(
            "UPDATE users SET \
             uploaded = COALESCE((SELECT sum(delta_up) FROM traffic_ledger WHERE user_id = users.id), 0), \
             downloaded = COALESCE((SELECT sum(delta_down) FROM traffic_ledger WHERE user_id = users.id), 0) \
             WHERE id = ANY($1)",
        )
        .bind(&users)
        .execute(db)
        .await?;
    }
    let torrents: Vec<i64> = touched_torrents.into_iter().collect();
    if !torrents.is_empty() {
        // 回填种子做种/下载计数（详情页与保种规则数据源）
        sqlx::query(
            "UPDATE torrents t SET \
             seeders = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.seeding), 0), \
             leechers = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.leeching), 0), \
             times_completed = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL), 0) \
             WHERE t.id = ANY($1)",
        )
        .bind(&torrents)
        .execute(db)
        .await?;
    }
    Ok(applied)
}

/// 单事件计费（促销裁决 + snatch upsert + 流水 + 保种时长累计）
/// 返回 torrent_id 供快照点刷收集（种子不存在返回 None）。
/// seed_cap：做种时长单次累计容忍窗（秒）= 2 × announce_interval，由 consume_announce 按站点设定算出
async fn process_event(
    db: &PgPool,
    ev: &AnnounceEvent,
    seed_cap: i64,
) -> anyhow::Result<Option<i64>> {
    // 未知种子的查询失败必须显式报错（重试），不能静默丢弃计费
    let torrent_id: Option<i64> =
        sqlx::query_scalar("SELECT id FROM torrents WHERE info_hash = $1")
            .bind(&ev.hash)
            .fetch_optional(db)
            .await?;
    let Some(torrent_id) = torrent_id else {
        return Ok(None); // 种子确实不存在：跳过
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

    // 0073 券倍率叠加：free 券 → 下载计 0；neutral 券 → 上下行均计 0。
    // 判定口径：本人该种存在绑定中（used_at 仍 NULL）的对应 kind 券。
    // 与促销取更优（乘法叠加：促销 x2 上传对 neutral 也归零，取对用户更优的 0）。
    let voucher: Option<String> = sqlx::query_scalar(
        "SELECT kind FROM user_vouchers \
         WHERE user_id = $1 AND used_torrent_id = $2 AND used_at IS NULL AND expires_at > now() \
         ORDER BY CASE kind WHEN 'neutral' THEN 2 WHEN 'free' THEN 1 ELSE 0 END DESC LIMIT 1",
    )
    .bind(ev.user)
    .bind(torrent_id)
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
        INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, last_up, last_down, leeching, seeding, completed_at, last_seen_at, connectable)
        VALUES ($1, $2, $3, $4, $5, $6, CASE WHEN $11 THEN FALSE ELSE $7 END, CASE WHEN $11 THEN FALSE ELSE $8 END, CASE WHEN $9 THEN now() ELSE NULL END, now(), COALESCE($12, 1))
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
    .bind(ev.conn)
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
    Ok(Some(torrent_id))
}

/// 复活任务自动验收（0073，U3D Graveyard 口径）：领取者补种累计时长 ≥ required_hours
/// 且当前仍在做种 → 发奖（火花 + 1 枚免费券）+ 种子挂 7 天 free bump + 站内信。
/// 幂等：状态 CAS（open→done），奖励只随成功转移发放一次。
async fn resurrection_settle(db: &PgPool) -> anyhow::Result<u64> {
    let settled = sqlx::query(
        r#"
        WITH done AS (
            UPDATE resurrections r SET status = 'done', finished_at = now()
            WHERE r.status = 'open'
              AND EXISTS (SELECT 1 FROM snatches s
                          WHERE s.user_id = r.user_id AND s.torrent_id = r.torrent_id
                            AND s.seeded_seconds >= r.required_hours * 3600 AND s.seeding)
            RETURNING r.id, r.user_id, r.torrent_id, r.reward_sparks
        )
        SELECT d.id, d.user_id, d.torrent_id, d.reward_sparks FROM done d
        "#,
    )
    .fetch_all(db)
    .await?;
    for row in &settled {
        let (rid, uid, tid, reward): (i64, i64, i64, i64) = (
            row.try_get(0)?,
            row.try_get(1)?,
            row.try_get(2)?,
            row.try_get(3)?,
        );
        let idem = format!("resurrection:{rid}");
        sqlx::query(
            r#"
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), $1, $2, 'resurrection', $3
            WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)
            "#,
        )
        .bind(uid)
        .bind(reward)
        .bind(&idem)
        .execute(db)
        .await?;
        sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
            .bind(uid)
            .bind(reward)
            .execute(db)
            .await?;
        sqlx::query(
            "INSERT INTO user_vouchers (user_id, kind, source) VALUES ($1, 'free', 'resurrection')",
        )
        .bind(uid)
        .execute(db)
        .await?;
        // 7 天 free bump（U3D 口径）：全站看见的即时激励
        sqlx::query(
            "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source) \
             VALUES ('torrent', $1, 'free', now(), now() + interval '7 days', 'task') \
             ON CONFLICT DO NOTHING",
        )
        .bind(tid)
        .execute(db)
        .await?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(uid)
        .bind("复活任务完成")
        .bind(format!(
            "恭喜！你复活种子 #{tid} 的任务已验收：奖励 {reward} 火花 + 1 枚免费券，\
             该种子已获得 7 天免费促销。感谢你为保种做出的贡献！"
        ))
        .execute(db)
        .await;
    }
    if !settled.is_empty() {
        tracing::info!(n = settled.len(), "resurrections settled");
    }
    Ok(settled.len() as u64)
}

/// 教材愿望单推送（0074，U3D WishList 教育化）：扫过去 1 小时过审的种子，
/// 对 wishlist 做 ILIKE/维度匹配；每条愿望 24h 限推一次，**每用户聚合一封信**（防信箱轰炸）。
async fn wishlist_notify(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH recent AS (
            SELECT id, name FROM torrents
            WHERE approval_status = 1
              AND approved_at > now() - interval '1 hour'
              AND approved_at IS NOT NULL
        ),
        hits AS (
            SELECT w.id AS wish_id, w.user_id, r.id AS torrent_id, r.name AS torrent_name
            FROM wishlist w
            JOIN recent r ON r.name ILIKE '%' || w.keyword || '%'
            WHERE (w.category_id IS NULL OR w.category_id = (SELECT category_id FROM torrents t WHERE t.id = r.id))
              AND (w.grade_id IS NULL OR w.grade_id = (SELECT grade_id FROM torrents t WHERE t.id = r.id))
              AND (w.notified_at IS NULL OR w.notified_at < now() - interval '24 hours')
        ),
        agg AS (
            SELECT user_id, string_agg('#' || torrent_id || ' ' || torrent_name, E'
' ORDER BY torrent_id) AS body,
                   bool_or(wish_id IS NOT NULL) AS dummy
            FROM hits GROUP BY user_id
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, user_id, '愿望单命中：你关注的新资源已上架', '你订阅的关键词有新种子过审：

' || body || '

（每条愿望 24 小时内只提醒一次；可在「我的 → 愿望单」管理订阅）'
        FROM agg
        WHERE dummy
        RETURNING 1
        "#,
    )
    .fetch_all(db)
    .await?;
    // 推送成功后刷新命中愿望的 notified_at（24h 节流）
    sqlx::query(
        r#"
        UPDATE wishlist w SET notified_at = now()
        FROM recent r
        WHERE r.id IN (SELECT id FROM torrents WHERE approval_status = 1 AND approved_at > now() - interval '1 hour' AND approved_at IS NOT NULL)
          AND r.name ILIKE '%' || w.keyword || '%'
          AND (w.notified_at IS NULL OR w.notified_at < now() - interval '24 hours')
        "#,
    )
    .execute(db)
    .await?;
    if !res.is_empty() {
        tracing::info!(n = res.len(), "wishlist notifications sent");
    }
    Ok(res.len() as u64)
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
          -- 0072 buffer 豁免（U3D hitrun.buffer 口径）：下载量不足种子 10% 视为误触/秒删，不计 H&R
          AND s.downloaded > t.size / 10
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

    // 3.5) 预警（0072，U3D prewarn 口径）：48h 内到期、未达标、未预警过的 → 站内信提醒。
    //      处罚前的缓冲带：教育站新人多，一次 PM 能挡掉大部分无意违规。
    let prewarned = sqlx::query(
        r#"
        WITH due AS (
            UPDATE hr_snapshots SET prewarned_at = now(), updated_at = now()
            WHERE status = 'open' AND prewarned_at IS NULL
              AND seeded_seconds < required_seconds
              AND deadline < now() + interval '48 hours'
            RETURNING user_id, torrent_id, seeded_seconds, required_seconds, deadline
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, d.user_id,
               'H&R 预警：请尽快补足做种',
               format('你完成的种子 #%s 距 H&R 考察截止还剩不到 48 小时（截止 %s）。当前累计做种 %s 小时，'
                      '需 %s 小时。请尽快恢复做种；也可在「我的 H&R」页用火花自助免罪。',
                      d.torrent_id,
                      to_char(d.deadline AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM-DD HH24:MI'),
                      round(d.seeded_seconds / 3600.0, 1),
                      round(d.required_seconds / 3600.0, 1))
        FROM due d
        "#,
    )
    .execute(db)
    .await?;
    if prewarned.rows_affected() > 0 {
        tracing::info!(n = prewarned.rows_affected(), "H&R pre-warnings sent");
    }

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
/// 0072 晋升待遇：升级时按 class_rules.promo_sparks 发放火花 + 系统消息（NP 升级送邀请口径；
/// 幂等键 class_promo:{user}:{new_class}，用户重复升降只补发差额档不重复入账）。
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
        RETURNING u.id, u.class_id AS old_class, t.new_class
        "#,
    )
    .fetch_all(db)
    .await?;
    if !promoted.is_empty() {
        tracing::info!(n = promoted.len(), "class promoted");
    }
    // 晋升待遇发放（0072）：promo_sparks > 0 的档位才发；幂等键防重复
    for row in &promoted {
        let (uid, old_class, new_class): (i64, i32, i32) =
            (row.try_get(0)?, row.try_get(1)?, row.try_get(2)?);
        let reward: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(promo_sparks),0) FROM class_rules \
             WHERE class_id > $1 AND class_id <= $2",
        )
        .bind(old_class)
        .bind(new_class)
        .fetch_one(db)
        .await?;
        if reward <= 0 {
            continue;
        }
        let idem = format!("class_promo:{}:{}", uid, new_class);
        let credited = sqlx::query(
            r#"
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), $1, $2, 'class_promotion', $3
            WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)
            "#,
        )
        .bind(uid)
        .bind(reward)
        .bind(&idem)
        .execute(db)
        .await?
        .rows_affected();
        if credited > 0 {
            sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1")
                .bind(uid)
                .bind(reward)
                .execute(db)
                .await?;
            let level_name: String =
                sqlx::query_scalar("SELECT name FROM class_rules WHERE class_id = $1")
                    .bind(new_class)
                    .fetch_optional(db)
                    .await?
                    .unwrap_or_else(|| format!("LV{new_class}"));
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 SELECT NULL, $1, $2, $3 WHERE u_notice_enabled($1, 'class_promo')",
            )
            .bind(uid)
            .bind("等级晋升祝贺")
            .bind(format!(
                "恭喜晋升至「{level_name}」！系统发放晋升奖励 {reward} 火花，已入账。\
                 继续保持做种与分享，更高等级还有更多奖励。"
            ))
            .execute(db)
            .await;
        }
    }
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

/// 闲置账号停用（0072，U3D AutoDisableInactiveUsers 口径的教育站收敛版）：
/// 90 天未登录、无任何做种、非员工（class<90）且非捐赠者 → dormant_at 打标。
/// 不改 status（保留封禁语义）、不删数据；登录侧拦截 dormant_at 非空者并提示联系管理组。
/// 排除 dormant_at 已打标（幂等）与 90 天内注册的新号（新人宽限）。
async fn dormant_mark(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        UPDATE users u SET dormant_at = now()
        WHERE u.class_id < 90 AND NOT u.donor AND u.dormant_at IS NULL
          AND u.created_at < now() - interval '90 days'
          AND COALESCE(u.last_seen_at, u.created_at) < now() - interval '90 days'
          AND NOT EXISTS (SELECT 1 FROM snatches s WHERE s.user_id = u.id AND s.seeding)
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "dormant accounts marked");
    }
    Ok(res.rows_affected())
}

/// P0-2 分区预建：为三张 RANGE 流水表预建 [当月, +2 月] 的月分区（每日一次，IF NOT EXISTS 幂等）。
/// 存量拆分见迁移 0070；没有本 job 时数据会持续落 default 分区导致裁剪失效。
pub async fn ensure_partitions(db: &PgPool) -> anyhow::Result<()> {
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    let first_of_month = now_site.date_naive().with_day(1).unwrap();
    let tables = [
        ("traffic_ledger", "window_start"),
        ("spark_ledger", "created_at"),
        ("posts", "created_at"),
    ];
    for (tbl, col) in tables {
        for i in 0..3i32 {
            let start = first_of_month + chrono::Duration::days(30 * i as i64);
            // 用 date_trunc 语义对齐月首（+30 天近似在月末附近可能漂移，改为逐次取下月一号）
            let start = first_of_month
                .checked_add_months(chrono::Months::new(i as u32))
                .unwrap_or(start);
            let end = start
                .checked_add_months(chrono::Months::new(1))
                .unwrap_or(start);
            let name = format!("{}_{}", tbl, start.format("%Y_%m"));
            sqlx::query(&format!(
                "CREATE TABLE IF NOT EXISTS {} PARTITION OF {} FOR VALUES FROM ('{}') TO ('{}')",
                name,
                tbl,
                start.format("%Y-%m-%d"),
                end.format("%Y-%m-%d")
            ))
            .execute(db)
            .await?;
            let _ = col; // 分区键仅作文档提示
        }
    }
    Ok(())
}

/// P0-6 种子级 up/down 差额对账（NP cheaterbox 口径）：
/// 虚报上传者没有对应真实下载方，同种子 7 天窗口 Σ(delta_up) − Σ(delta_down) 长期为正且巨大。
/// 免费促销（free/x2free，含全局/官种/分类维度）天然产生差额，豁免。
/// 命中 → cheat_events（agent='torrent_gap', agent 字段存 torrent:{id}）+ 首次进管理组信箱。
pub async fn cheat_audit(db: &PgPool) -> anyhow::Result<u64> {
    let threshold_gb: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'cheat_gap_threshold_gb'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(50)
    .clamp(1, 10240);
    let threshold = threshold_gb * 1024 * 1024 * 1024;

    let rows: Vec<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT torrent_id, SUM(delta_up) - SUM(delta_down) AS gap
        FROM traffic_ledger
        WHERE window_start > now() - interval '7 days'
        GROUP BY torrent_id
        HAVING SUM(delta_up) - SUM(delta_down) > $1 AND COUNT(DISTINCT user_id) >= 2
        "#,
    )
    .bind(threshold)
    .fetch_all(db)
    .await?;

    let mut first_hits = 0u64;
    for (torrent_id, gap) in rows {
        // 免费促销豁免（与 hr_enforce 的免费判定同口径）
        let exempt: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM promotions p \
             WHERE (p.torrent_id = $1 OR p.torrent_id IS NULL) \
               AND p.starts_at <= now() AND p.ends_at > now() \
               AND (p.kind IN ('free','x2free') OR (p.scope = 'official' AND EXISTS (SELECT 1 FROM torrents t WHERE t.id = $1 AND t.official_tag))))",
        )
        .bind(torrent_id)
        .fetch_one(db)
        .await
        .unwrap_or(false);
        if exempt {
            continue;
        }
        let agent = format!("torrent:{torrent_id}");
        let existed: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM cheat_events WHERE user_id = 0 AND agent = $1 AND reason = 'torrent_gap_audit')",
        )
        .bind(&agent)
        .fetch_one(db)
        .await
        .unwrap_or(true);
        sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, reason) VALUES (0, $1, 'torrent_gap_audit') \
             ON CONFLICT (user_id, agent, reason) DO UPDATE SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(&agent)
        .execute(db)
        .await?;
        if !existed {
            first_hits += 1;
            let body = format!(
                "种子 #{torrent_id} 近 7 天上传总量比下载总量高出约 {gap} 字节（阈值 {threshold_gb} GiB），\
                 存在虚报上传（假流量）嫌疑。明细见后台「作弊探测」，请人工核对做种者列表。"
            );
            let _: Result<_, _> = sqlx::query(
                "INSERT INTO staffmessages (user_id, subject, body, permission) \
                 SELECT MIN(id), '流量差额审计告警', $1, 'cheater' FROM users WHERE class_id >= 90",
            )
            .bind(body)
            .execute(db)
            .await;
        }
    }
    if first_hits > 0 {
        tracing::warn!(n = first_hits, "cheat_audit: new torrent-gap suspects");
    }
    Ok(first_hits)
}

/// P1-8 Ratio Watch（GZ 口径柔性观察期）：
/// 分享率跌破阈值 → 一次性站内信警告 + 14 天观察期；期内恢复自动解除，到期仍跌破 → 停下载权并通知管理组。
/// 阈值/期限走 site_settings：ratio_watch_threshold（默认 0.4）、ratio_watch_days（默认 14）。
pub async fn ratio_watch(db: &PgPool) -> anyhow::Result<u64> {
    let threshold_f: f64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'ratio_watch_threshold'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<f64>().ok())
    .unwrap_or(0.4)
    .clamp(0.01, 10.0);
    let days: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'ratio_watch_days'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(14)
    .clamp(1, 90);

    // ① 新跌破者：进入观察期 + 一次性警告信
    let entered = sqlx::query(
        r#"
        WITH low AS (
            SELECT id, username, uploaded, downloaded FROM users
            WHERE status < 2 AND class_id < 90 AND downloaded > 0 AND ratio_watch_until IS NULL
              AND uploaded::float8 / downloaded::float8 < $1
        ),
        entered AS (
            UPDATE users u SET ratio_watch_until = now() + make_interval(days => $2), ratio_warned_at = now()
            FROM low WHERE u.id = low.id RETURNING low.id, low.username
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, id, '分享率警告（观察期）',
               '您好 ' || username || '，您的分享率已跌破站点警戒线。请在 ' || $2 || ' 天内恢复（做种/发布均可提升上传量），' ||
               '观察期结束仍未恢复将暂停下载权限。如有特殊情况请联系管理组。'
        FROM entered
        "#,
    )
    .bind(threshold_f)
    .bind(days)
    .execute(db)
    .await?
    .rows_affected();

    // ② 到期仍跌破：暂停下载 + 通知管理组（管理组可手动恢复 download_enabled）
    let punished = sqlx::query(
        r#"
        WITH expired AS (
            UPDATE users SET download_enabled = FALSE
            WHERE status < 2 AND class_id < 90 AND download_enabled
              AND ratio_watch_until IS NOT NULL AND ratio_watch_until < now()
              AND downloaded > 0 AND uploaded::float8 / downloaded::float8 < $1
            RETURNING id, username
        )
        INSERT INTO staffmessages (user_id, subject, body, permission)
        SELECT id, 'Ratio Watch 到期处置', '用户 ' || username || '（#' || id || '）观察期结束仍未恢复分享率，已按规则暂停下载权限，请人工复核。', 'cheater'
        FROM expired
        "#,
    )
    .bind(threshold_f)
    .execute(db)
    .await?
    .rows_affected();

    // ③ 期内恢复：自动解除观察
    let cleared = sqlx::query(
        "UPDATE users SET ratio_watch_until = NULL \
         WHERE ratio_watch_until IS NOT NULL \
           AND (downloaded = 0 OR uploaded::float8 / downloaded::float8 >= $1)",
    )
    .bind(threshold_f)
    .execute(db)
    .await?
    .rows_affected();

    if entered + punished + cleared > 0 {
        tracing::info!(entered, punished, cleared, "ratio_watch cycle");
    }
    Ok(entered + punished)
}

/// P2-11 登录 IP 跳变检测（/24 段近似）：24h 内单账号登录来源超过阈值个不同 /24 段 → 记录作弊探测。
/// 纯近似（无 GeoIP 依赖，教育网 DHCP 换段是常态），只记录供人工参考，不自动处置。
pub async fn multi_ip_check(db: &PgPool) -> anyhow::Result<u64> {
    let threshold: i64 = 8;
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT user_id, COUNT(DISTINCT split_part(host(ip), '.', 1) || '.' || split_part(host(ip), '.', 2) || '.' || split_part(host(ip), '.', 3)) AS segments
        FROM login_events
        WHERE created_at > now() - interval '24 hours' AND ip IS NOT NULL AND host(ip) LIKE '%.%'
        GROUP BY user_id
        HAVING COUNT(DISTINCT split_part(host(ip), '.', 1) || '.' || split_part(host(ip), '.', 2) || '.' || split_part(host(ip), '.', 3)) >= $1
        "#,
    )
    .bind(threshold)
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for (user_id, segments) in rows {
        sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, reason) VALUES ($1, 'login_ip_spread', 'multi_subnet_24h') \
             ON CONFLICT (user_id, agent, reason) DO UPDATE SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(user_id)
        .execute(db)
        .await?;
        let _ = segments;
        n += 1;
    }
    Ok(n)
}

/// P0-1 兜底纠偏：全量重算 users/torrents 快照（每 6h 一次）。
/// 快照权威在流水；增量化后的点刷可能因历史漂移累积误差，低频全量对账收敛。
pub async fn reconcile_snapshots(db: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE users SET \
         uploaded = COALESCE((SELECT sum(delta_up) FROM traffic_ledger WHERE user_id = users.id), 0), \
         downloaded = COALESCE((SELECT sum(delta_down) FROM traffic_ledger WHERE user_id = users.id), 0)",
    )
    .execute(db)
    .await?;
    sqlx::query(
        "UPDATE torrents t SET \
         seeders = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.seeding), 0), \
         leechers = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.leeching), 0), \
         times_completed = COALESCE((SELECT count(*) FROM snatches s WHERE s.torrent_id = t.id AND s.completed_at IS NOT NULL), 0)",
    )
    .execute(db)
    .await?;
    Ok(())
}

/// 主循环：定时任务调度。
pub async fn run_all(db: PgPool, mut redis: redis::aio::ConnectionManager) -> anyhow::Result<()> {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut hour_tick = tokio::time::interval(std::time::Duration::from_secs(3600));
    let mut first_hour = true;
    let mut last_bank_day: Option<chrono::NaiveDate> = None;
    // 0071 反作弊/性能调度
    let mut tick10 = tokio::time::interval(std::time::Duration::from_secs(600));
    let mut tick30 = tokio::time::interval(std::time::Duration::from_secs(1800));
    let mut tick6h = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
    let mut tick1d = tokio::time::interval(std::time::Duration::from_secs(24 * 3600));
    let mut first_tick10 = true;
    let mut first_tick30 = true;
    let mut first_tick6h = true;
    let mut first_tick1d = true;
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
                if let Err(e) = ratio_watch(&db).await { tracing::error!(?e, "ratio_watch"); }
                if let Err(e) = dormant_mark(&db).await { tracing::error!(?e, "dormant_mark"); }
                if let Err(e) = wishlist_notify(&db).await { tracing::error!(?e, "wishlist_notify"); }
                if let Err(e) = resurrection_settle(&db).await { tracing::error!(?e, "resurrection_settle"); }
            }
            _ = tick10.tick() => {
                if first_tick10 { first_tick10 = false; continue; }
                if let Err(e) = cheat_audit(&db).await { tracing::error!(?e, "cheat_audit"); }
            }
            _ = tick30.tick() => {
                if first_tick30 { first_tick30 = false; continue; }
                if let Err(e) = multi_ip_check(&db).await { tracing::error!(?e, "multi_ip_check"); }
            }
            _ = tick6h.tick() => {
                if first_tick6h { first_tick6h = false; continue; }
                if let Err(e) = reconcile_snapshots(&db).await { tracing::error!(?e, "reconcile_snapshots"); }
            }
            _ = tick1d.tick() => {
                if first_tick1d { first_tick1d = false; continue; }
                if let Err(e) = ensure_partitions(&db).await { tracing::error!(?e, "ensure_partitions"); }
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

    /// 0074 做种收益公式口径锁定：与 SQL 内 per_torrent/per_user 完全同构的 Rust 镜像。
    /// 防止有人只改 SQL 不改文档（或反之）；数值变了必须同步注释与「收益说明」前端文案。
    fn mirror(rule_bonus: f64, seeders: i64, seed_hours: f64) -> f64 {
        rule_bonus / (seeders.max(1) as f64).powf(0.35) / (1.0 + seed_hours / 2160.0)
    }
    fn user_hourly(base: f64, torrents: &[(f64, i64, f64)]) -> f64 {
        let bonus: f64 = torrents.iter().map(|(b, s, h)| mirror(*b, *s, *h)).sum();
        base + 400.0 / std::f64::consts::PI * (bonus * 6.0 / 50.0).atan()
    }

    #[test]
    fn seeding_formula_rarity_decay() {
        // 同样种子：1 人做种 vs 10 人做种，收益显著衰减（Gazelle seeders^-0.35）
        let lone = user_hourly(10.0, &[(0.25, 1, 0.0)]);
        let crowd = user_hourly(10.0, &[(0.25, 10, 0.0)]);
        assert!(crowd < lone * 0.85, "crowd={crowd} lone={lone}");
    }

    #[test]
    fn seeding_formula_time_saturation() {
        // 同样种子：新做种 vs 已挂 90 天（2160h），收益约减半（半衰设计）
        let fresh = user_hourly(10.0, &[(0.25, 1, 0.0)]);
        let aged = user_hourly(10.0, &[(0.25, 1, 2160.0)]);
        let ratio = aged / fresh; // 含底薪稀释；纯加成部分约减半
        assert!((0.75..0.95).contains(&ratio), "ratio={ratio}");
    }

    #[test]
    fn seeding_formula_arctan_cap() {
        // 1000 个濒危种也只有软封顶：趋近 base+200，不线性涨到天上（NP arctan 口径）
        let many: Vec<(f64, i64, f64)> = (0..1000).map(|_| (2.0, 1, 0.0)).collect();
        let total = user_hourly(10.0, &many);
        assert!(total < 10.0 + 210.0, "total={total}");
        assert!(total > 10.0 + 100.0, "total={total}（应明显超过半程）");
    }

    #[test]
    fn seeding_formula_dying_beats_daily() {
        // 濒危保种（bonus 2.0）时薪高于日常种（0.25）——激励方向正确
        let dying = user_hourly(10.0, &[(2.0, 1, 0.0)]);
        let daily = user_hourly(10.0, &[(0.25, 1, 0.0)]);
        assert!(dying > daily * 1.8);
    }

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
