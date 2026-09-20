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
                    // 首次命中 → 按 agent_hit_action 分级（U5 §12.3）：
                    // log=仅 staffmessages 告警（现状 T3）/ warn=告警+用户警告信
                    let action: String = sqlx::query_scalar::<_, String>(
                        "SELECT value FROM site_settings WHERE name = 'agent_hit_action'",
                    )
                    .fetch_optional(db)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "log".into());
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
                    if action == "warn" {
                        let _: Result<_, _> = sqlx::query(
                            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                             VALUES (NULL, $1, '客户端不合规警告', $2)",
                        )
                        .bind(ev.user)
                        .bind(format!(
                            "您使用的客户端（{}）不符合本站允许名单（原因：{}）。请更换为允许的客户端，否则将无法继续做种/下载。如有疑问请联系管理组。",
                            if agent.is_empty() { "(空)" } else { &agent },
                            reason,
                        ))
                        .execute(db)
                        .await;
                    }
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
/// 审计修复：保留一年历史窗口——hr_enforce 建 H&R 快照时要按 completed_at 时点
/// 回查「当时是否处于免费窗口」豁免，物理删掉近期促销会让回查失明（误判违规）。
/// 计费查询全部走 `starts_at <= now() < ends_at` 生效窗口，不受历史保留影响。
pub async fn expire_promotions(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query("DELETE FROM promotions WHERE ends_at <= now() - interval '365 days'")
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

/// 保种认领结算（审计修复 P1 闭环补全）：claim 记了 seed_time_begin/uploaded_begin
/// 基线但无任何结算方——「认领→奖励」断裂为展示层。口径（NP claims 按时长发奖）：
///   每满 24h 有效保种（结算时点仍在做种）发 preserve_bonus_per_day（缺省 100，
///   site_settings 可调）；幂等键 = preserve:{torrent_id}:{day_index}（自认领起算的
///   整天序号），重跑/补跑安全。停做种期间不结算（恢复后在下个整天边界继续），
/// 时长基线列保留供后台展示 delta。
pub async fn preserve_settle(db: &PgPool) -> anyhow::Result<u64> {
    let bonus: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'preserve_bonus_per_day')::bigint, 100)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(100);
    let res = sqlx::query(
        r#"
        WITH due AS (
            SELECT sp.claimed_by AS user_id, sp.torrent_id,
                   floor(EXTRACT(EPOCH FROM (now() - sp.claimed_at)) / 86400)::bigint AS day_index
            FROM seed_preserve sp
            JOIN snatches s
              ON s.torrent_id = sp.torrent_id AND s.user_id = sp.claimed_by AND s.seeding
            WHERE sp.claimed_by IS NOT NULL AND sp.exited_at IS NULL
        ),
        fresh AS (
            SELECT d.* FROM due d
            WHERE d.day_index >= 1 AND NOT EXISTS (
                SELECT 1 FROM spark_ledger l
                WHERE l.idempotency_key = 'preserve:' || d.torrent_id || ':' || d.day_index
            )
        )
        INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key)
        SELECT nextval('spark_ledger_id_seq'), user_id, $1, 'preserve_reward', 'torrent',
               torrent_id, 'preserve:' || torrent_id || ':' || day_index
        FROM fresh
        "#,
    )
    .bind(bonus)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 做种收益小时结算（0074 改造：稀有度衰减 + 时长饱和，NP/U3D 口径融合）。
/// 每用户每小时一条流水，幂等键 = seeding:{user}:{yyyymmddhh}。
///
/// 做种收益结算（每小时一次，全站批量）。
///
/// 公式（**公式本体在 DB 函数里**，见迁移 0133，worker/API 共用同一真相源）：
///   每颗种子 s_i = ( 种子档位分 + 个人做种时长档位分 )
///                × min(1, log10(1+size_GB)/log10(1+vol_base))   -- 体积对数饱和
///                × (1 + rarity_k × seeders^-rarity_exp)         -- 稀有度加成
///                × scale                                        -- 总标定
///   时魔 = ( base + floor( (2/π)·cap·atan(Σ × curve_k) ) ) × (donor ? donor_mult : 1)
///
/// 档位分：
///   种子维度（第一命中优先）—— 濒危 2.0（seeders≤1 且 完成≥3）/ 高龄 1.5（>365天）/ 老种 1.0（>180天）
///                            / 大体积 0.75（≥100GiB）/ 中体积 0.5（≥25GiB）/ 日常 0.25
///   个人做种时长 —— ≥1年 +2.0 / 6-12月 +1.0 / 3-6月 +0.75 / 1-3月 +0.5 / 不足1月 0
///
/// 改造要点（2026-09-19，对照 NP/Gazelle/U3D 后）：① 补体积因子堵「堆小种」套利
/// （旧公式 1000 颗 1MB 能拿 ~205/h，比保 6 颗濒危大种还高）；② 个人做种时长从
/// 「衰减乘数」改为「加分档位」（三家都奖励长期保种，我们此前方向相反）；
/// ③ 稀有度从「只惩罚热门」改为「加成独苗」（seeders 大时仍趋近 1）。
/// 参数全部走 site_settings（`seeding_*`），标签/曲线可热调。标定见迁移注释。
///
/// 反假保种（0071）：回连不可达且从无上传的做种行不计。
/// 反假保种（0071）：回连不可达且从无上传的做种行不计。
/// 数据新鲜度：`last_seen_at` 超过僵尸阈值（max(2h, 2×announce_interval)）的行不计 ——
/// 客户端崩溃/卸载不会发 stopped 事件，`sweep_stale_peers` 虽会清理标记，但那是另一个 tick
/// 的任务；结算不该依赖"另一个 job 恰好跑过"，所以这里独立过滤一次（同一阈值函数）。
pub async fn seeding_reward(db: &PgPool, stale_secs: i64) -> anyhow::Result<u64> {
    let hour = chrono::Utc::now().format("%Y%m%d%H").to_string();
    let res = sqlx::query(
        r#"
        INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
        SELECT nextval('spark_ledger_id_seq'), d.id, d.amount, 'seeding_reward', d.idem
        FROM (
            -- MATERIALIZED：参数行必须只求值一次。若不显式物化，优化器可能把
            -- seeding_params() 内联进 nestloop 内层 → 每颗种子读 8 次 site_settings。
            WITH p AS MATERIALIZED (SELECT * FROM seeding_params()),
            per_torrent AS (
                SELECT u.id AS user_id, u.donor,
                       seeding_torrent_bonus(
                           t.size,
                           t.seeders,
                           (EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0)::double precision,
                           t.times_completed,
                           (GREATEST(s.seeded_seconds, 0) / 3600.0)::double precision,
                           p.vol_base, p.rarity_k, p.rarity_exp, p.scale
                       ) AS b
                FROM users u
                JOIN snatches s ON s.user_id = u.id AND s.seeding
                JOIN torrents t ON t.id = s.torrent_id
                  AND NOT (s.connectable::int = 0 AND s.uploaded = 0)
                CROSS JOIN p
                WHERE u.status < 2
                  AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
            ),
            per_user AS (
                SELECT user_id, donor, sum(b) AS bonus_raw
                FROM per_torrent GROUP BY user_id, donor
            )
            SELECT pu.user_id AS id,
                   seeding_hourly(pu.bonus_raw, p.base, p.cap, p.curve_k, p.donor_mult, pu.donor) AS amount,
                   'seeding:' || pu.user_id || ':' || $1 AS idem
            FROM per_user pu CROSS JOIN p
        ) d
        WHERE NOT EXISTS (
            SELECT 1 FROM spark_ledger l WHERE l.idempotency_key = d.idem
        )
        "#,
    )
    .bind(&hour)
    .bind(stale_secs)
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
    // 刷新余额快照（权威在流水，快照仅展示）。
    // 行锁串行化：与 API 的「锁行读余额→插流水→改余额」事务互斥，避免聚合快照
    // 覆写并发事务刚落账的余额（丢失更新）；单事务内先锁后算，聚合与更新一致。
    // 审计修复（P0 锁表）：旧版 `WHERE id IN (SELECT id FROM users FOR UPDATE)` 每小时
    // 对全表行加锁，与所有写余额的 API 事务互斥（用户量大时持锁秒级）。改为只重算
    // 本小时流水覆盖到的用户 —— sum(ledger) 结果与其余用户现有快照一致，语义不变。
    sqlx::query(
        "UPDATE users SET spark_balance = COALESCE((             SELECT sum(amount) FROM spark_ledger WHERE user_id = users.id          ), 0) \
         WHERE id IN (SELECT DISTINCT user_id FROM spark_ledger WHERE created_at > now() - interval '2 hours')",
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
    /// tracker 收到 announce 的时点（RFC3339）。促销按「事件时点」而非「消费时点」裁决：
    /// 事件积压（DLQ 重试/worker 停机）时避免免费窗口结束后按原价补计费。
    #[serde(default)]
    ts: Option<chrono::DateTime<chrono::Utc>>,
    /// BT 客户端 UA（0098 下载列表「客户端」列）；截断 200
    #[serde(default)]
    agent: String,
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
                let _ = redis::cmd("DEL")
                    .arg(format!("flux:announce:fail:{id}"))
                    .query_async::<()>(&mut redis.clone())
                    .await;
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
                // 处理失败：不推进游标，下轮重试（避免丢失计费）。
                // 审计修复（P0，v2）：持久性错误会永久卡死整条流。原实现的连续失败计数是
                // 函数局部变量——consume_announce 每分钟被独立调用一次，计数每轮归零，
                // 熔断永远触发不了。改为 Redis 键 flux:announce:fail:{id} 持久化计数：
                // 同一 ID 连续失败 6 次进死信并推进游标，事件本体留在 DLQ 供人工补偿计费。
                let fail_key = format!("flux:announce:fail:{id}");
                let streak: i64 = redis::cmd("INCR")
                    .arg(&fail_key)
                    .query_async::<i64>(&mut redis.clone())
                    .await
                    .unwrap_or(1);
                let _ = redis::cmd("EXPIRE")
                    .arg(&fail_key)
                    .arg(3600)
                    .query_async::<()>(&mut redis.clone())
                    .await;
                tracing::error!(%id, ?e, streak, "事件计费失败，游标暂停等待重试");
                if streak >= 6 {
                    tracing::error!(%id, "同一事件连续 6 次失败，转入死信队列并跳过（flux:announce:dlq）");
                    let mut conn_dl = redis_dead_letter.clone();
                    let payload_txt = payload.clone();
                    let _: Result<(), _> = redis::cmd("RPUSH")
                        .arg("flux:announce:dlq")
                        .arg(format!("{id}	{payload_txt}"))
                        .query_async(&mut conn_dl)
                        .await;
                    let _ = redis::cmd("DEL")
                        .arg(&fail_key)
                        .query_async::<()>(&mut redis.clone())
                        .await;
                    applied += 1;
                    last_seen_id = Some(id);
                    continue;
                }
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
    // 审计修复（P1）：announce 哈希是客户端「原始字节」口径；库内 info_hash 为规范化
    // 重编码口径（键序非排序的种子两者不同，此前静默丢计费）。双口径 OR 匹配。
    let torrent: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, COALESCE(size, 0) FROM torrents WHERE info_hash = $1 OR raw_info_hash = $1",
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
    let (up_mult, down_mult) = billing_multipliers(kind.as_deref(), global.as_deref());

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
                    "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'speed_alarm_bps')::bigint, 2147483648)",
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

/// 盒子/高速做种打标（0077，U3D AutoHighspeedTag 口径）：近 7 天 snatches 上传统计速度
/// 超 100MB/s 视为高速线路在做种 → torrents.highspeed = true（展示与激励用，不惩罚）。
async fn highspeed_tag(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH speeds AS (
            -- 平均上速 = 累计上传 / max(做种秒数, 1h)：对短时突发鲁棒（U3D 用墙钟，我们做种口径更准）
            SELECT torrent_id, max(uploaded / GREATEST(seeded_seconds::bigint, 3600)) AS bps
            FROM snatches WHERE uploaded > 0 AND last_seen_at > now() - interval '7 days'
            GROUP BY torrent_id
        )
        UPDATE torrents t SET highspeed = TRUE
        FROM speeds s WHERE t.id = s.torrent_id AND s.bps > 104857600 AND NOT t.highspeed
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "highspeed torrents tagged");
    }
    Ok(res.rows_affected())
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
              AND r.team_id IS NULL
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
        // 奖励链整段包进单事务（含幂等护栏）：CAS 已置 done 后崩溃，
        // 重启重跑此循环仍能凭幂等键补发，不再永久丢奖励。
        let mut tx = db.begin().await?;
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
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)")
            .bind(uid)
            .bind(reward)
                .bind(&idem)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO user_vouchers (user_id, kind, source) \
             SELECT $1, 'free', 'resurrection' \
             WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $2)",
        )
        .bind(uid)
        .bind(&idem)
        .execute(&mut *tx)
        .await?;
        // 7 天 free bump（U3D 口径）：全站看见的即时激励
        sqlx::query(
            "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source) \
             VALUES ('torrent', $1, 'free', now(), now() + interval '7 days', 'task') \
             ON CONFLICT DO NOTHING",
        )
        .bind(tid)
        .execute(&mut *tx)
        .await?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(uid)
        .bind("复活任务完成")
        .bind(format!(
            "恭喜！你复活种子 #{tid} 的任务已验收：奖励 {reward} 魔力 + 1 枚免费券，\
             该种子已获得 7 天免费促销。感谢你为保种做出的贡献！"
        ))
        .execute(&mut *tx)
        .await;
        tx.commit().await?;
    }
    if !settled.is_empty() {
        tracing::info!(n = settled.len(), "resurrections settled");
    }
    Ok(settled.len() as u64)
}

/// 组队契约结算（0102 社交层）：团队成员在契约期内的做种增量合计达标 →
/// 按贡献占比分账 + 写拯救荣誉 + 推进状态。
///
/// 与单人复活任务**严格隔离**：resurrection_settle 只处理 `team_id IS NULL` 的行，
/// 否则队长会按 `r.user_id` 再拿一份全额奖励（双发）。
///
/// 判据：团队总增量 ≥ required_hours × 3600，且至少一名成员当前仍在做种。
/// 贡献口径：snatches 增量（`seeded_seconds - 基线`），与 seed_preserve 同款 —— 精度到单资源，
/// 杜绝「挂无关种子刷契约贡献」；也不用 spark_ledger 反查（seeding_reward 按用户逐小时聚合、无 ref_id）。
///
/// 可靠性：以 `social_team.settled_at IS NULL` 作为「未处理」标记（而非纯状态 CAS），
/// 配合 spark_ledger 幂等键 `social:team:{team_id}:{uid}` —— 中途崩溃重跑既不丢奖也不重发。
async fn social_team_settle(db: &PgPool) -> anyhow::Result<u64> {
    let teams: Vec<(i64, i64, i64, i64, i32)> = sqlx::query_as(
        r#"
        SELECT st.id, r.id, r.torrent_id, r.reward_sparks, r.required_hours
        FROM social_team st
        JOIN resurrections r ON r.team_id = st.id AND r.status = 'open'
        WHERE st.status IN (0, 1) AND st.settled_at IS NULL
          AND EXISTS (
              SELECT 1 FROM social_team_member m
              JOIN snatches s ON s.user_id = m.uid AND s.torrent_id = r.torrent_id
              WHERE m.team_id = st.id AND m.join_status IN (0, 1) AND s.seeding)
          AND (
              SELECT COALESCE(sum(GREATEST(COALESCE(s.seeded_seconds, 0)::bigint - m.seed_seconds_begin, 0)), 0)
              FROM social_team_member m
              LEFT JOIN snatches s ON s.user_id = m.uid AND s.torrent_id = r.torrent_id
              WHERE m.team_id = st.id AND m.join_status IN (0, 1)
          ) >= r.required_hours::bigint * 3600
        "#,
    )
    .fetch_all(db)
    .await?;

    if teams.is_empty() {
        return Ok(0);
    }

    // 信誉参数：完成/失败/退出都由系统判定，不可伪造（队友评价不参与主流程，防互刷）
    let rep_gain: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name = 'social_rep_on_fulfilled'), 20)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(20);
    let rep_min: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name = 'social_rep_min'), 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let rep_max: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name = 'social_rep_max'), 2000)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(2000);

    let mut done = 0u64;
    for (team_id, rid, tid, reward, _hours) in &teams {
        let (team_id, rid, tid, reward) = (*team_id, *rid, *tid, *reward);
        let mut tx = db.begin().await?;

        let members: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT m.uid, GREATEST(COALESCE(s.seeded_seconds, 0)::bigint - m.seed_seconds_begin, 0) \
             FROM social_team_member m \
             LEFT JOIN snatches s ON s.user_id = m.uid AND s.torrent_id = $2 \
             WHERE m.team_id = $1 AND m.join_status IN (0, 1)",
        )
        .bind(team_id)
        .bind(tid)
        .fetch_all(&mut *tx)
        .await?;

        let total: i64 = members.iter().map(|m| m.1).sum();
        if total <= 0 {
            continue;
        }

        // 按增量占比分账；整数除法的余数落在排序末位（贡献最少者）身上，
        // 保证分配总额恰好等于 reward（实测 5000 → 2777 + 2223）
        let mut ordered = members.clone();
        ordered.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut remaining = reward;
        for (i, (uid, delta)) in ordered.iter().enumerate() {
            let amount = if i == ordered.len() - 1 {
                remaining
            } else {
                (reward * delta) / total
            };
            remaining -= amount;
            if amount > 0 {
                let idem = format!("social:team:{team_id}:{uid}");
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key) \
                     SELECT nextval('spark_ledger_id_seq'), $1, $2, 'social_team_reward', $3 \
                     WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)",
                )
                .bind(*uid)
                .bind(amount)
                .bind(&idem)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 \
                     AND NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)",
                )
                .bind(*uid)
                .bind(amount)
                .bind(&idem)
                .execute(&mut *tx)
                .await?;
            }
            sqlx::query(
                "UPDATE social_team_member SET contributed_sec = $3, settled_amount = $4, join_status = 4 \
                 WHERE team_id = $1 AND uid = $2",
            )
            .bind(team_id)
            .bind(*uid)
            .bind(delta)
            .bind(amount)
            .execute(&mut *tx)
            .await?;
        }

        // 拯救荣誉（永久留存，不随赛季重置）
        let uids: Vec<i64> = members.iter().map(|m| m.0).collect();
        sqlx::query(
            "INSERT INTO rescue_honor (torrent_id, info_hash, team_id, uids, total_sec, seeders_before, seeders_after) \
             SELECT $1, t.info_hash, $2, $3, $4, 0, COALESCE(t.seeders, 0) FROM torrents t WHERE t.id = $1",
        )
        .bind(tid)
        .bind(team_id)
        .bind(&uids)
        .bind(total)
        .execute(&mut *tx)
        .await?;

        // 状态推进：settled_at 是「未处理」标记，必须与发奖同事务置位
        sqlx::query("UPDATE social_team SET status = 2, settled_at = now() WHERE id = $1")
            .bind(team_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE resurrections SET status = 'done', finished_at = now() WHERE id = $1")
            .bind(rid)
            .execute(&mut *tx)
            .await?;

        for (uid, _) in &members {
            // 信誉 +（契约完成是系统判定的事实，不可伪造）
            sqlx::query(
                "INSERT INTO social_reputation (uid, score, fulfilled_count, updated_at) \
                 VALUES ($1, 1000 + $2, 1, now()) \
                 ON CONFLICT (uid) DO UPDATE SET \
                   score = LEAST($3, GREATEST($4, social_reputation.score + $2)), \
                   fulfilled_count = social_reputation.fulfilled_count + 1, \
                   updated_at = now()",
            )
            .bind(*uid)
            .bind(rep_gain)
            .bind(rep_max)
            .bind(rep_min)
            .execute(&mut *tx)
            .await?;
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(*uid)
            .bind("保种协作完成")
            .bind(format!(
                "你们小队协作保种的资源 #{tid} 已验收，奖励按贡献分摊到账。感谢你为保种出的力！"
            ))
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await?;
        done += 1;
    }

    if done > 0 {
        tracing::info!(n = done, "social teams settled");
    }
    Ok(done)
}

/// 组队契约超时失败（0103）：到期仍未达成 → 判失败。
///
/// 设计口径（见 `_doc/契约失败流转与信誉.md`）：
///   * **不连坐**：失败是团队的共同结果，不额外惩罚个别成员；只记一次失败事实（信誉小减）
///   * 与成功结算互斥：本 job 排在 social_team_settle **之后**执行，恰好卡在期限内达标的仍算成功；
///     事务内再用 `status IN (0,1) AND settled_at IS NULL` 做 CAS，防止被结算抢先
///   * 与「中途退出」区分：到期未达标**不算逃跑**，不套用退出惩罚——惩罚只针对主动跑掉的人
async fn social_team_expire(db: &PgPool) -> anyhow::Result<u64> {
    let rep_delta: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name = 'social_rep_on_failed'), -5)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(-5);
    let rep_min: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name = 'social_rep_min'), 0)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let rep_max: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name = 'social_rep_max'), 2000)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(2000);

    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM social_team \
         WHERE status IN (0, 1) AND settled_at IS NULL \
           AND deadline_at IS NOT NULL AND deadline_at < now()",
    )
    .fetch_all(db)
    .await?;

    let mut n = 0u64;
    for team_id in &due {
        let mut tx = db.begin().await?;
        // CAS：与成功结算互斥（结算 job 排在前面，已把 settled_at 置位的队伍在此被跳过）
        let claimed = sqlx::query(
            "UPDATE social_team SET status = 3, fail_reason = 'deadline_exceeded', settled_at = now() \
             WHERE id = $1 AND status IN (0, 1) AND settled_at IS NULL",
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if claimed == 0 {
            continue;
        }

        let tid: Option<i64> =
            sqlx::query_scalar("SELECT torrent_id FROM resurrections WHERE team_id = $1 LIMIT 1")
                .bind(team_id)
                .fetch_optional(&mut *tx)
                .await?;

        sqlx::query(
            "UPDATE resurrections SET status = 'expired', finished_at = now() \
             WHERE team_id = $1 AND status = 'open'",
        )
        .bind(team_id)
        .execute(&mut *tx)
        .await?;

        let members: Vec<i64> = sqlx::query_scalar(
            "SELECT uid FROM social_team_member WHERE team_id = $1 AND join_status IN (0, 1)",
        )
        .bind(team_id)
        .fetch_all(&mut *tx)
        .await?;

        for uid in &members {
            sqlx::query(
                "INSERT INTO social_reputation (uid, score, failed_count, updated_at) \
                 VALUES ($1, 1000 + $2, 1, now()) \
                 ON CONFLICT (uid) DO UPDATE SET \
                   score = LEAST($3, GREATEST($4, social_reputation.score + $2)), \
                   failed_count = social_reputation.failed_count + 1, \
                   updated_at = now()",
            )
            .bind(*uid)
            .bind(rep_delta)
            .bind(rep_max)
            .bind(rep_min)
            .execute(&mut *tx)
            .await?;

            // 文案只陈述事实 + 说明已产生的收益不受影响，不指责
            // （协作失败本就令人沮丧，不该再加羞辱）
            let body = match tid {
                Some(t) => format!(
                    "资源 #{t} 的协作保种未在期限内达标，契约已结束。\
                     你已产生的做种时长仍计入做种收益，不受影响。"
                ),
                None => "协作保种未在期限内达标，契约已结束。".to_string(),
            };
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(*uid)
            .bind("保种协作已到期")
            .bind(body)
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await?;
        n += 1;
    }

    if n > 0 {
        tracing::info!(n, "social teams expired");
    }
    Ok(n)
}

/// 教材愿望单推送（0074，U3D WishList 教育化）：扫过去 1 小时过审的种子，
/// 对 wishlist 做 ILIKE/维度匹配；每条愿望 24h 限推一次，**每用户聚合一封信**（防信箱轰炸）。
async fn wishlist_notify(db: &PgPool) -> anyhow::Result<u64> {
    // 审计修复（原子性）：INSERT 消息与 UPDATE notified_at 包进同一事务——
    // 两条独立语句中途崩溃会出现「发了信却没标记」（下轮重发）或「标记了却没发信」
    // （该愿望 24h 内彻底漏推）的错位。事务内两语句同生共死。
    let mut tx = db.begin().await?;
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
    .fetch_all(&mut *tx)
    .await?;
    // 24h 节流推进（审计修复：原第二条独立 UPDATE 引用上一条语句的 CTE `recent`，
    // 每轮报 relation "recent" does not exist，notified_at 永不推进 → 命中窗口内每小时重发。
    // 改为同一事务内重算同构 CTE 后推进，与 INSERT 同生共死，杜绝"发了信却没标记"的错位）
    let _ = sqlx::query(
        r#"
        WITH recent AS (
            SELECT id, name FROM torrents
            WHERE approval_status = 1
              AND approved_at > now() - interval '1 hour'
              AND approved_at IS NOT NULL
        ),
        hits AS (
            SELECT w.id AS wish_id, w.user_id
            FROM wishlist w
            JOIN recent r ON r.name ILIKE '%' || w.keyword || '%'
            WHERE (w.category_id IS NULL OR w.category_id = (SELECT category_id FROM torrents t WHERE t.id = r.id))
              AND (w.grade_id IS NULL OR w.grade_id = (SELECT grade_id FROM torrents t WHERE t.id = r.id))
              AND (w.notified_at IS NULL OR w.notified_at < now() - interval '24 hours')
        )
        UPDATE wishlist w SET notified_at = now()
        WHERE w.id IN (SELECT wish_id FROM hits)
        "#,
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
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
          -- 审计修复：档位判定改用累计做种秒数（与 H&R/seeding_reward 同口径）。
          -- 旧墙钟口径「完成至今的挂机时长」会把只下载不做种的账号也计入里程碑。
          AND s.seeded_seconds >= h.hours * 3600
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
          AND s.downloaded > t.size * 104 / 1000   -- 与券核销同阈值（≈10.4%）：10%~10.4% 区间不再误判
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
                      '需 %s 小时。请尽快恢复做种；也可在「我的 H&R」页用魔力自助免罪。',
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
        ),
        ins AS (
            INSERT INTO hr_violations (user_id, torrent_id, seeded_seconds, required_seconds)
            SELECT user_id, torrent_id, seeded_seconds, required_seconds FROM dead
            ON CONFLICT DO NOTHING
            RETURNING user_id, torrent_id, seeded_seconds, required_seconds
        )
        -- 审计修复（P1）：violated 此前只落表+日志零成本躺平。补 PM 告知违规与免罪途径
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, ins.user_id, 'H&R 违规确认',
               '种子 #' || ins.torrent_id || ' 的 H&R 考察期已结束且未达标（做种 '
               || round(ins.seeded_seconds / 3600.0, 1) || ' 小时 / 要求 '
               || round(ins.required_seconds / 3600.0, 1) || ' 小时），已记违规一次。
               持续做种可自行恢复；也可在「我的 H&R」用 20000 魔力自助免罪。累计多次违规将影响下载权限。'
        FROM ins
        "#,
    )
    .execute(db)
    .await?;
    if violated.rows_affected() > 0 {
        tracing::warn!(n = violated.rows_affected(), "H&R violations detected");
        // 违规行同步 snatches.hr_flag（/me/hr 与列表角标口径）
        let _ = sqlx::query(
            "UPDATE snatches s SET hr_flag = TRUE FROM hr_violations v \
             WHERE v.user_id = s.user_id AND v.torrent_id = s.torrent_id AND NOT s.hr_flag",
        )
        .execute(db)
        .await;
    }

    // 5) hr_flag 刷新（0029 一次性迁移的运行时延续）：完成已超 14 天且做种时长 < 120h。
    //    此前该标记只在迁移里置过一次，运行时无人刷新 —— /me/hr（community_http）口径失真。
    sqlx::query(
        // 审计修复（P1）：硬编码 14 天/120 小时与 hr_policy 可配口径脱节，逐种取 policy
        "UPDATE snatches s SET hr_flag = TRUE \
         FROM torrents t WHERE t.id = s.torrent_id AND s.completed_at IS NOT NULL \
           AND s.seeded_seconds < COALESCE((t.hr_policy->>'seed_hours')::int, 48) * 3600 \
           AND s.completed_at < now() - make_interval(days => COALESCE((t.hr_policy->>'days')::int, 14)) \
           AND NOT s.hr_flag",
    )
    .execute(db)
    .await?;
    Ok(())
}

/// H&R 违规处罚执行点（审计修复：hr_enforce 此前只落表+PM 零成本躺平）。
/// 未解决违规数（hr_violations.resolved_at IS NULL）≥ hr_violation_limit（默认 3）
///   → users.download_enabled = false + PM 说明自助免罪路径；
/// 违规数降回阈值以下 → 恢复 download_enabled = true。
/// 口径取舍（注释存档）：是否「因 H&R 被禁」不引入新列/PM 反查——违规数一旦低于阈值
/// 就恢复下载，可能顺带恢复因其他原因（如 Ratio Watch 到期处置）被禁的用户。
/// 取舍理由：Ratio Watch 侧管理组手动恢复是主路径，此处自动恢复保住多数用户的体验；
/// 若需精确归因，可后续为 users 增加 ban 原因位图。豁免 hr.exempt / 员工不处罚。
async fn hr_punish(db: &PgPool) -> anyhow::Result<()> {
    let limit: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'hr_violation_limit'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(3)
    .clamp(1, 100);

    // ① 超限 → 暂停下载 + PM（仅本轮新被禁的发信，幂等靠 download_enabled 翻转）
    let punished = sqlx::query(
        r#"
        WITH viol AS (
            SELECT v.user_id, count(*) AS n
            FROM hr_violations v
            WHERE v.resolved_at IS NULL
            GROUP BY v.user_id
            HAVING count(*) >= $1
        ),
        banned AS (
            UPDATE users u
            SET download_enabled = FALSE
            FROM viol
            WHERE u.id = viol.user_id
              AND u.status < 2
              AND u.download_enabled
              AND NOT user_can(u.id, 'hr.exempt')
            RETURNING u.id, viol.n
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, b.id, '下载权限暂停：H&R 违规超限',
               format('您当前有 %s 条未解决的 H&R 违规（阈值 %s），已暂停下载权限。'
                      '恢复方式：①持续做种达标后违规自动消除；②在「我的 H&R」页用魔力自助免罪；'
                      '③联系管理组申请 Pardon。违规数降回阈值以下后下载权限将自动恢复。',
                      b.n, $1)
        FROM banned b
        "#,
    )
    .bind(limit)
    .execute(db)
    .await?;
    if punished.rows_affected() > 0 {
        tracing::warn!(
            n = punished.rows_affected(),
            limit,
            "H&R 违规超限，已暂停下载权限"
        );
    }

    // ② 降回阈值以下 → 自动恢复下载。
    // 口径注释：不区分当初被禁原因（见函数头取舍说明）——违规数低于阈值即恢复，
    // 极小概率把其他原因禁用的账号一并恢复，换取 H&R 自助闭环不依赖人工。
    let restored = sqlx::query(
        r#"
        UPDATE users u
        SET download_enabled = TRUE
        WHERE u.status < 2
          AND NOT u.download_enabled
          AND COALESCE((SELECT count(*) FROM hr_violations v
                        WHERE v.user_id = u.id AND v.resolved_at IS NULL), 0) < $1
          AND NOT EXISTS (
              -- Ratio Watch 到期处置仍生效的用户不在此恢复（那边由管理组/观察期自愈管理）
              SELECT 1 FROM users u2
              WHERE u2.id = u.id AND u2.ratio_watch_until IS NOT NULL AND u2.ratio_watch_until < now()
          )
        "#,
    )
    .bind(limit)
    .execute(db)
    .await?;
    if restored.rows_affected() > 0 {
        tracing::info!(
            n = restored.rows_affected(),
            "H&R 违规降回阈值以下，已恢复下载权限"
        );
    }
    Ok(())
}

/// 死种入保种区（审计修复：保种区此前无数据源，页面恒空）。
/// `seeders=0 AND leechers=0 AND approval_status=1 AND created_at < now() - preserve_dead_days`
/// 且不在 seed_preserve 表中的种子 INSERT（claimed_by 为 NULL，等待认领）。
/// preserve_dead_days 默认 7（site_settings，迁移 0084 播种）。
async fn preserve_seed(db: &PgPool) -> anyhow::Result<u64> {
    let dead_days: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'preserve_dead_days'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(7)
    .clamp(1, 365);
    let res = sqlx::query(
        r#"
        INSERT INTO seed_preserve (torrent_id)
        SELECT t.id
        FROM torrents t
        WHERE t.seeders = 0 AND t.leechers = 0
          AND t.approval_status = 1
          AND t.created_at < now() - make_interval(days => $1::int)
          AND NOT EXISTS (SELECT 1 FROM seed_preserve sp WHERE sp.torrent_id = t.id)
        ON CONFLICT (torrent_id) DO NOTHING
        "#,
    )
    .bind(dead_days as i32)
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), dead_days, "死种入保种区");
    }
    Ok(res.rows_affected())
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
            "SELECT COALESCE(sum(promo_sparks),0)::bigint FROM class_rules \
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
            sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)")
                .bind(uid)
                .bind(reward)
                .bind(&idem)
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
                "恭喜晋升至「{level_name}」！系统发放晋升奖励 {reward} 魔力，已入账。\
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

/// 僵尸 peer 判定阈值（秒）= max(2h, 2×announce_interval)。
/// 同时用于两处：① `sweep_stale_peers` 清理标记；② `seeding_reward` 结算前的数据新鲜度过滤
/// （结算是发钱动作，不能依赖"另一个 job 恰好跑过"）。
async fn stale_peer_threshold_secs(db: &PgPool) -> i64 {
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
    (interval * 2).max(7200)
}

/// 僵尸做种/下载标记清理：tracker peer 表 90s 超时即除名，但 DB 侧 snatches.seeding/leeching
/// 原本只在下一次 announce 时被覆盖 —— 客户端崩溃/卸载（无 stopped 事件）的行会永久保持
/// seeding=true，导致 seeding_reward 空转发钱（live 证据：27 行 last_seen 2 天前仍在领收益）
/// 与 torrents.seeders 虚高。阈值 = max(2h, 2×announce_interval)，远大于正常重汇报抖动。
async fn sweep_stale_peers(db: &PgPool) -> anyhow::Result<u64> {
    let threshold_secs = stale_peer_threshold_secs(db).await;
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

/// 过期邀请落库回收（NP docleanup 口径）：status=0 且过期的邀请码统一置 status=2。
/// 此前仅展示层 CASE 折算，库内 status 恒 0——按 status 统计的后台口径失真。
async fn expire_invites(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query("UPDATE invites SET status = 2 WHERE status = 0 AND expires_at <= now()")
        .execute(db)
        .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "expired invites recycled");
    }
    Ok(res.rows_affected())
}

/// 一次性凭证清理：download_keys（30 分钟）与 password_resets（30 分钟）过期即删。
/// password_resets 原本只在手动 POST /admin/docleanup 里清，无人点击则永久堆积。
async fn purge_expired_tokens(db: &PgPool) -> anyhow::Result<u64> {
    let a = sqlx::query("DELETE FROM download_keys WHERE expires_at < now()")
        .execute(db)
        .await?
        .rows_affected();
    let b = sqlx::query("DELETE FROM password_resets WHERE expires_at < now()")
        .execute(db)
        .await?
        .rows_affected();
    Ok(a + b)
}

/// announce 死信队列可见性（卫生 P1）：DLQ 只进不出等于变相丢计费。
/// 有积压时通知管理组信箱（复用 cheat_audit 告警模式），同一批积压只告警一次。
async fn dlq_watch(db: &PgPool, redis: &mut redis::aio::ConnectionManager) -> anyhow::Result<u64> {
    use redis::AsyncCommands;
    let len: i64 = redis.llen("flux:announce:dlq").await.unwrap_or(0);
    if len == 0 {
        // 队列清空后复位告警游标，下批积压可再次告警
        let _: () = redis.del("flux:announce:dlq:alerted").await.unwrap_or(());
        return Ok(0);
    }
    let alerted: i64 = redis.get("flux:announce:dlq:alerted").await.unwrap_or(0);
    if alerted == 0 {
        let body = format!(
            "announce 死信队列当前积压 {len} 条事件（连续失败 6 次进入），计费已跳过。\
             请排查 flux:announce:dlq 并人工补偿计费。"
        );
        let _: Result<_, _> = sqlx::query(
            "INSERT INTO staffmessages (user_id, subject, body, permission) \
             SELECT MIN(id), 'announce 死信队列积压告警', $1, 'cheater' FROM users WHERE class_id >= 90",
        )
        .bind(body)
        .execute(db)
        .await;
        let _: () = redis
            .set_ex("flux:announce:dlq:alerted", 1, 24 * 3600)
            .await
            .unwrap_or(());
        tracing::warn!(len, "announce DLQ backlog alerted");
    }
    Ok(len as u64)
}

/// 成就授予（0079 G6，U3D 口径教育站收敛版）：四族指标聚合 → 达标授予 + 火花奖励。
/// 幂等：PK (user_id, def_id) 天然防重；奖励走幂等键 achievement:{def_code}:{uid}。
async fn achievement_grant(db: &PgPool) -> anyhow::Result<u64> {
    let _res = sqlx::query(
        r#"
        WITH metrics AS (
            SELECT u.id AS user_id,
                   COALESCE(u.seeding_size, 0) AS seeding_bytes,
                   (SELECT count(*) FROM resurrections r WHERE r.user_id = u.id AND r.status = 'done') AS rescue_count,
                   (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS upload_count,
                   (SELECT count(*) FROM posts p WHERE p.user_id = u.id) AS post_count
            FROM users u WHERE u.status < 2
        ),
        m AS (
            SELECT user_id, 'seeding_bytes' AS metric, seeding_bytes AS val FROM metrics
            UNION ALL SELECT user_id, 'rescue_count', rescue_count FROM metrics
            UNION ALL SELECT user_id, 'upload_count', upload_count FROM metrics
            UNION ALL SELECT user_id, 'post_count', post_count FROM metrics
        ),
        due AS (
            SELECT m.user_id, d.id AS def_id, d.code, d.reward_sparks, m.val
            FROM m JOIN achievement_defs d ON d.metric = m.metric AND m.val >= d.threshold
        )
        INSERT INTO user_achievements (user_id, def_id, metric_value)
        SELECT user_id, def_id, val FROM due
        ON CONFLICT (user_id, def_id) DO NOTHING
        "#,
    )
    .fetch_all(db)
    .await?;
    // RETURNING 只能引用目标表列（code/reward_sparks 属于 achievement_defs）。
    // 0079 上线以来因 RETURNING 语法错误，成就系统从未授予过 —— 改为插入后反查达标行。
    let res: Vec<(i64, i64, String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT ua.user_id, ua.def_id, d.code, d.reward_sparks, ua.metric_value
        FROM user_achievements ua
        JOIN achievement_defs d ON d.id = ua.def_id
        WHERE (ua.user_id, ua.def_id) IN (
            SELECT m.user_id, d2.id FROM (
                SELECT u.id AS user_id,
                       COALESCE(u.seeding_size, 0) AS seeding_bytes,
                       (SELECT count(*) FROM resurrections r WHERE r.user_id = u.id AND r.status = 'done') AS rescue_count,
                       (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS upload_count,
                       (SELECT count(*) FROM posts p WHERE p.user_id = u.id) AS post_count
                FROM users u WHERE u.status < 2
            ) m JOIN achievement_defs d2 ON (
                (d2.metric = 'seeding_bytes' AND m.seeding_bytes >= d2.threshold) OR
                (d2.metric = 'rescue_count'  AND m.rescue_count  >= d2.threshold) OR
                (d2.metric = 'upload_count'   AND m.upload_count   >= d2.threshold) OR
                (d2.metric = 'post_count'     AND m.post_count     >= d2.threshold))
        )
        "#,
    )
    .fetch_all(db)
    .await?;
    for &(uid, def_id, ref code, reward, _val) in &res {
        if reward > 0 {
            let idem = format!("achievement:{code}:{uid}");
            // 审计修复（原子性）：INSERT 流水与 UPDATE 余额包进同一事务——
            // 两语句分离时中途崩溃会出现「流水已落、余额未加」（或反之），对账永久撕裂。
            // 幂等键护栏保持：事务内 NOT EXISTS 防任务重跑双发。
            let mut tx = db.begin().await?;
            sqlx::query(
                r#"
                INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
                SELECT nextval('spark_ledger_id_seq'), $1, $2, 'achievement', $3
                WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)
                "#,
            )
            .bind(uid)
            .bind(reward)
            .bind(&idem)
            .execute(&mut *tx)
            .await?;
            // 审计修复（幂等）：UPDATE 与 INSERT 的 NOT EXISTS 同护栏 ——
            // 否则每小时任务重跑时流水幂等跳过、余额却再加一次（每用户每小时白得 200）
            sqlx::query(
                "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 \
                 AND NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)",
            )
            .bind(uid)
            .bind(reward)
            .bind(&idem)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        let _ = def_id;
    }
    if !res.is_empty() {
        tracing::info!(n = res.len(), "achievements grant pass done");
    }
    // 新授予（本轮才落 user_achievements 的行）补发站内信；历史达标行只走上面的
    // 幂等发奖路径，不重复发信（修复每小时向全部历史达标成就重发通知的轰炸）。
    let newly: Vec<(i64, String, i64)> = sqlx::query_as(
        r#"
        SELECT ua.user_id, d.code, d.reward_sparks
        FROM user_achievements ua
        JOIN achievement_defs d ON d.id = ua.def_id
        WHERE (ua.user_id, ua.def_id) IN (
            SELECT m.user_id, d2.id FROM (
                SELECT u.id AS user_id,
                       COALESCE(u.seeding_size, 0) AS seeding_bytes,
                       (SELECT count(*) FROM resurrections r WHERE r.user_id = u.id AND r.status = 'done') AS rescue_count,
                       (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1) AS upload_count,
                       (SELECT count(*) FROM posts p WHERE p.user_id = u.id) AS post_count
                FROM users u WHERE u.status < 2
            ) m JOIN achievement_defs d2 ON (
                (d2.metric = 'seeding_bytes' AND m.seeding_bytes >= d2.threshold) OR
                (d2.metric = 'rescue_count'  AND m.rescue_count  >= d2.threshold) OR
                (d2.metric = 'upload_count'   AND m.upload_count   >= d2.threshold) OR
                (d2.metric = 'post_count'     AND m.post_count     >= d2.threshold))
        )
        AND NOT EXISTS (
            SELECT 1 FROM messages msg
            WHERE msg.receiver_id = ua.user_id
              AND msg.subject = '成就达成'
              AND msg.body LIKE '%「' || d.code || '」%'
        )
        "#,
    )
    .fetch_all(db)
    .await?;
    for &(uid, ref code, reward) in &newly {
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
        )
        .bind(uid)
        .bind("成就达成")
        .bind(format!("恭喜达成成就「{code}」！奖励 {reward} 魔力已入账。"))
        .execute(db)
        .await;
    }
    if !newly.is_empty() {
        tracing::info!(n = newly.len(), "achievements granted");
    }
    Ok(res.len() as u64)
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

/// 绩效考核月末结算（0106，参考各 PT 站工作组考核口径）。
///
/// 每天 tick 一次，幂等护栏：
///   * 只处理**上一期**（站点时区 UTC+8 已跨月）`source='admin'` 且 `settled_at IS NULL` 的登记行
///   * 「settled_at IS NULL 作未处理标记 + 状态推进与发奖同一事务」（social_team_settle 同款，
///     崩溃整体回滚下轮重来；比旧 resurrection_settle 的「先 CAS 置 done 后发奖」可靠）
///   * 发薪幂等键 `jixiao:settle:{claim_id}`，先查后插防重复
///
/// 结算语义：
///   * 达标（min_requirements 全部满足）→ status=1 + 发 base_pay+加成（达标月数含本期）
///   * 未达标 → status=2 + 平实文案 PM（不羞辱：已产生的做种收益不受影响）
///   * 同轮为下一期落全站活跃用户基线快照（jixiao_baseline_snapshots，
///     PK(user_id,period) 幂等）——compute_metrics 的 seed_hours 等差值口径依赖它
///   * 本 job 不检查任何模块开关（已分配的岗位必须能收尾，social 层同款约定）
///
/// 注意：用户在补领窗口（jixiao_claim_window_days，默认 7 天）内仍可自领当期
/// ——worker 结算先到先得，两边都以「settled_at IS NULL + 单事务 CAS」互斥。
pub async fn jixiao_settle(db: &PgPool) -> anyhow::Result<u64> {
    // 站点时区（UTC+8）当前月份；只有跨月后上一期才可结算
    let now_site = chrono::Utc::now() + chrono::Duration::hours(8);
    let cur: String = now_site.format("%Y-%m").to_string();
    let prev: String = {
        let (y, m): (i32, u32) = {
            let mut it = cur.split('-');
            let y = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let m = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            (y, m)
        };
        if y == 0 || !(1..=12).contains(&m) {
            return Ok(0);
        }
        if m == 1 {
            format!("{:04}-12", y - 1)
        } else {
            format!("{y:04}-{:02}", m - 1)
        }
    };

    // 上一期未结算的 admin 登记行（含用户/岗位信息）
    #[derive(sqlx::FromRow)]
    struct Pending {
        id: i64,
        user_id: i64,
        type_id: i64,
        name: String,
        base_pay: i64,
        min_requirements: serde_json::Value,
        /// 岗位级加成配置（0106：admin 表单写入；缺省回落全站配置）
        #[sqlx(default)]
        bonus_rules: serde_json::Value,
        base_seed_seconds: i64,
        base_uploaded: i64,
        base_uploads: i64,
    }
    let pending: Vec<Pending> = sqlx::query_as(
        "SELECT c.id, c.user_id, c.type_id, t.name, t.base_pay, t.min_requirements, t.bonus_rules, \
                c.base_seed_seconds, c.base_uploaded, c.base_uploads \
         FROM jixiao_claims c JOIN jixiao_types t ON t.id = c.type_id \
         WHERE c.period = $1 AND c.metrics_snapshot->>'source' = 'admin' \
           AND c.settled_at IS NULL AND c.status = 0",
    )
    .bind(&prev)
    .fetch_all(db)
    .await?;
    if pending.is_empty() {
        // 没有登记行也要保证基线快照滚动（新月份第一次 tick 落当期快照）
        return jixiao_rollover_snapshot(db, &cur).await.map(|_| 0);
    }

    // 加成参数：全站 site_settings 兜底（岗位级 bonus_rules 优先，逐岗位在循环内取）
    let site_step: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name='jixiao_bonus_months_per_step'), 3)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(3)
    .max(1);
    let site_pct: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings WHERE name='jixiao_bonus_percent_per_step'), 10)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(10);

    let mut done = 0u64;
    for p in &pending {
        let mut tx = db.begin().await?;

        // 用户当前指标（期末值；基线 = 登记行 base_*，期初快照表缺它兜底——
        // 与 API compute_metrics 同优先级：快照表为主，这里登记行在期内必然存在，
        // 且结算时快照表写的是「下期」，本期基线只能来自登记行/上期快照）
        let cur_vals: Option<(i64, i64, i64)> = sqlx::query_as(
            "SELECT u.uploaded, u.uploaded, \
                    (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1) \
             FROM users u WHERE u.id = $1",
        )
        .bind(p.user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((uploaded_now, _dup, uploads_now)) = cur_vals else {
            // 用户已删：登记行作废不结算（不扣不发，keep 事实行）
            sqlx::query("UPDATE jixiao_claims SET status = 3, settled_at = now() WHERE id = $1")
                .bind(p.id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            continue;
        };
        let seed_seconds_now: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(seeded_seconds),0)::bigint FROM snatches WHERE user_id = $1",
        )
        .bind(p.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let seed_hours = ((seed_seconds_now - p.base_seed_seconds).max(0)) / 3600;
        let uploads_delta = (uploads_now - p.base_uploads).max(0);
        let uploaded_delta = (uploaded_now - p.base_uploaded).max(0);
        let seed_days: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT date_trunc('day', s.last_seen_at)) FROM snatches s \
             WHERE s.user_id = $1 AND to_char(s.last_seen_at, 'YYYY-MM') = $2 AND s.seeding",
        )
        .bind(p.user_id)
        .bind(&prev)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let seeding_count: i64 = sqlx::query_scalar(
            "SELECT count(DISTINCT torrent_id) FROM snatches WHERE user_id = $1 AND seeding",
        )
        .bind(p.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let seed_size: i64 = sqlx::query_scalar(
            "SELECT COALESCE(sum(t.size),0)::bigint FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding",
        )
        .bind(p.user_id)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        let (up_month, down_month): (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(sum(delta_up),0)::bigint, COALESCE(sum(delta_down),0)::bigint \
             FROM traffic_ledger WHERE user_id = $1 AND to_char(window_start, 'YYYY-MM') = $2",
        )
        .bind(p.user_id)
        .bind(&prev)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or((0, 0));
        let ops: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM audit_log WHERE actor_id = $1 AND to_char(created_at, 'YYYY-MM') = $2",
        )
        .bind(p.user_id)
        .bind(&prev)
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);

        // 与 API compute_metrics 同键集合（结算快照留全量，判定只读 min_requirements）
        let metrics = serde_json::json!({
            "uploaded": up_month.max(uploaded_delta), "downloaded": down_month, "uploads": uploads_delta,
            "seeding_count": seeding_count, "seed_size": seed_size,
            "seed_size_tb": seed_size / 1_099_511_627_776,
            "seed_hours": seed_hours, "seed_days": seed_days, "ops": ops,
        });

        // 达标判定：min_requirements 全部满足（与 /jixiao/claim 同口径）
        let mut all_ok = true;
        if let Some(reqs) = p.min_requirements.as_object() {
            for (k, v) in reqs {
                let required = v.as_i64().unwrap_or(0);
                if required > 0 && metrics.get(k).and_then(|x| x.as_i64()).unwrap_or(0) < required {
                    all_ok = false;
                    break;
                }
            }
        }

        if all_ok {
            // 达标月数（含本期）：历史 status=1 行数 + 1
            let months_before: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM jixiao_claims WHERE user_id = $1 AND type_id = $2 AND status = 1",
            )
            .bind(p.user_id)
            .bind(p.type_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
            // 岗位级加成优先（jixiao_types.bonus_rules：admin 表单「每 N 月 +M%」），
            // 未配置/非法回落全站 site_settings——与 API 侧 jixiao_bonus 同口径
            let (step, pct) = match p.bonus_rules.as_object().map(|r| {
                (
                    r.get("months_per_step").and_then(|v| v.as_i64()),
                    r.get("percent_per_step").and_then(|v| v.as_i64()),
                )
            }) {
                Some((Some(s), Some(pc))) if s > 0 && pc >= 0 => (s, pc),
                _ => (site_step, site_pct),
            };
            let bonus = p.base_pay * pct / 100 * ((months_before + 1) / step);
            let total = p.base_pay + bonus;

            // CAS：与用户补领互斥（先到先得）
            let upd = sqlx::query(
                "UPDATE jixiao_claims SET status = 1, settled_at = now(), amount = $2, bonus_paid = $3, \
                        metrics_at_settle = $4, \
                        metrics_snapshot = metrics_snapshot || '{\"settle_by\":\"worker\"}'::jsonb \
                 WHERE id = $1 AND settled_at IS NULL AND status = 0",
            )
            .bind(p.id)
            .bind(total)
            .bind(bonus)
            .bind(&metrics)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if upd == 0 {
                tx.rollback().await?;
                continue;
            }
            if total > 0 {
                let idem = format!("jixiao:settle:{}", p.id);
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
                )
                .bind(&idem)
                .fetch_one(&mut *tx)
                .await?;
                if !exists {
                    let balance: i64 = sqlx::query_scalar(
                        "SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE",
                    )
                    .bind(p.user_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    sqlx::query(
                        "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
                         VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'jixiao_reward', $3, $4)",
                    )
                    .bind(p.user_id)
                    .bind(total)
                    .bind(&idem)
                    .bind(balance + total)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1",
                    )
                    .bind(p.user_id)
                    .bind(total)
                    .execute(&mut *tx)
                    .await?;
                }
            }
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(p.user_id)
            .bind("绩效考核工资已发放")
            .bind(format!(
                "你本期（{}）的「{}」考核已达标，工资 {} 魔力（含连续达标加成 {}）已自动发放到账。",
                prev, p.name, total, bonus
            ))
            .execute(&mut *tx)
            .await;
        } else {
            let upd = sqlx::query(
                "UPDATE jixiao_claims SET status = 2, settled_at = now(), metrics_at_settle = $2, \
                        metrics_snapshot = metrics_snapshot || '{\"settle_by\":\"worker\"}'::jsonb \
                 WHERE id = $1 AND settled_at IS NULL AND status = 0",
            )
            .bind(p.id)
            .bind(&metrics)
            .execute(&mut *tx)
            .await?
            .rows_affected();
            if upd == 0 {
                tx.rollback().await?;
                continue;
            }
            // 平实文案（不羞辱纪律：不指责，只陈述事实 + 收益不受影响的说明）
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES (NULL, $1, $2, $3)",
            )
            .bind(p.user_id)
            .bind("绩效考核本期未达标")
            .bind(format!(
                "你本期（{}）的「{}」考核未达到岗位指标，本期工资未发放。\
                 你已产生的做种/上传收益不受影响，下期继续。",
                prev, p.name
            ))
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await?;
        done += 1;
    }

    // 下一期（=当前期）基线快照：结算完毕后滚动
    jixiao_rollover_snapshot(db, &cur).await?;
    if done > 0 {
        tracing::info!(done, period = %prev, "jixiao settled");
    }
    Ok(done)
}

/// 为指定期落全站活跃用户基线快照（幂等：PK 冲突跳过——首个到达的快照即基线，
/// 重跑不覆盖：期初值必须固定，中途覆盖会让 delta 口径漂移）。
async fn jixiao_rollover_snapshot(db: &PgPool, period: &str) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        INSERT INTO jixiao_baseline_snapshots (user_id, period, seed_seconds, uploaded, uploads)
        SELECT u.id, $1,
               COALESCE((SELECT sum(s.seeded_seconds) FROM snatches s WHERE s.user_id = u.id), 0),
               u.uploaded,
               (SELECT count(*) FROM torrents tr WHERE tr.owner_id = u.id AND tr.approval_status = 1)
        FROM users u
        WHERE u.status < 2
          AND (EXISTS(SELECT 1 FROM snatches s2 WHERE s2.user_id = u.id)
               OR u.uploaded > 0)
        ON CONFLICT (user_id, period) DO NOTHING
        "#,
    )
    .bind(period)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
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
        // 免费促销豁免（与 hr_enforce 的免费判定同口径）。
        // 审计修复：官方范围（scope='official'）促销此前不看 kind 整条豁免——
        // 官方 x2（上传双倍、下载照计）也会把 up-down gap 洗成「免费」跳过审计。
        // 收窄为：仅下载侧免费类 kind（free/x2free）豁免（x2free 上传虽双倍，但下载为 0，
        // 天然产生 gap 且属官方促销口径，整条豁免）；其余 kind（x2/half/x2half/p30）一律不豁免。
        let exempt: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM promotions p \
             WHERE (p.torrent_id = $1 OR p.torrent_id IS NULL) \
               AND p.starts_at <= now() AND p.ends_at > now() \
               AND p.kind IN ('free','x2free'))",
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
            UPDATE users u SET ratio_watch_until = now() + make_interval(days => $2::int), ratio_warned_at = now()
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

    // ② 到期仍跌破：按 ratio_watch_action 分级处置（U5 §12.3：
    // warn=仅再警告 / limit_download=暂停下载+通知管理组，默认后者=现状 T3）
    let action: String = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'ratio_watch_action'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "limit_download".into());

    let (punished, notified) = if action == "warn" {
        let expired: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, username FROM users \
             WHERE status < 2 AND class_id < 90 AND download_enabled \
               AND ratio_watch_until IS NOT NULL AND ratio_watch_until < now() \
               AND downloaded > 0 AND uploaded::float8 / downloaded::float8 < $1",
        )
        .bind(threshold_f)
        .fetch_all(db)
        .await
        .unwrap_or_default();
        for (uid, uname) in &expired {
            let _ = sqlx::query(
                "INSERT INTO messages (sender_id, receiver_id, subject, body) \
                 VALUES (NULL, $1, '分享率警告（观察期已满）', $2)",
            )
            .bind(uid)
            .bind(format!(
                "您好 {uname}，您的观察期已结束但分享率仍未恢复至 {threshold_f:.2}。本次仅提醒；持续未恢复可能影响下载权限。"
            ))
            .execute(db)
            .await;
        }
        (expired.len() as u64, 0u64)
    } else {
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
        (punished, punished)
    };
    let _ = notified;

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

/// 对账告警（每 6h，先于 reconcile_snapshots 执行）：比对「流水聚合 vs 快照」，
/// 差异超阈值即 error 告警。reconcile 负责静默收敛快照漂移；本 job 负责「收敛前
/// 先留证」——付费下载不扣快照、捐赠套餐漏落流水这类「快照与流水背离」的缺陷，
/// 正是靠静默 reconcile 掩盖的（有流水注入路径绕过快照 = 真实账目差异）。
/// 注意：演示数据（0018）自带无流水的初始余额/流量，会稳定触发本告警——属真实
/// 差异信号，执行 0108 清理后消失。
pub async fn reconcile_diff_alert(db: &PgPool) -> anyhow::Result<()> {
    let (ledger_spark, snapshot_spark): (i64, i64) = sqlx::query_as(
        "SELECT \
           COALESCE((SELECT sum(amount) FROM spark_ledger), 0)::bigint, \
           COALESCE((SELECT sum(spark_balance) FROM users), 0)::bigint",
    )
    .fetch_one(db)
    .await?;
    let spark_diff = ledger_spark - snapshot_spark;
    // 阈值 ±1000：并发窗口内的正常漂移容忍；持续超限 = 存在绕过流水的动账路径
    if spark_diff.abs() > 1000 {
        tracing::error!(
            ledger = ledger_spark,
            snapshot = snapshot_spark,
            diff = spark_diff,
            "对账告警：spark_ledger 合计与 users.spark_balance 合计背离超阈值（存在流水外的余额变动源，请排查）"
        );
    }
    let (ledger_up, snapshot_up, ledger_down, snapshot_down): (i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
               COALESCE((SELECT sum(delta_up) FROM traffic_ledger), 0)::bigint, \
               COALESCE((SELECT sum(uploaded) FROM users), 0)::bigint, \
               COALESCE((SELECT sum(delta_down) FROM traffic_ledger), 0)::bigint, \
               COALESCE((SELECT sum(downloaded) FROM users), 0)::bigint",
        )
        .fetch_one(db)
        .await?;
    // 阈值 ±10GiB
    const TRAFFIC_TOLERANCE: i64 = 10 * 1024 * 1024 * 1024;
    for (name, ledger_v, snapshot_v) in [
        ("uploaded", ledger_up, snapshot_up),
        ("downloaded", ledger_down, snapshot_down),
    ] {
        let diff = ledger_v - snapshot_v;
        if diff.abs() > TRAFFIC_TOLERANCE {
            tracing::error!(
                field = name,
                ledger = ledger_v,
                snapshot = snapshot_v,
                diff = diff,
                "对账告警：traffic_ledger 合计与 users 快照背离超阈值（存在流水外的流量变动源，请排查）"
            );
        }
    }
    let negatives: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM users WHERE spark_balance < 0 OR uploaded < 0 OR downloaded < 0",
    )
    .fetch_one(db)
    .await?;
    if negatives > 0 {
        tracing::error!(
            count = negatives,
            "对账告警：{} 个用户快照为负值（超花/负流量，请核查动账路径）",
            negatives
        );
    }
    Ok(())
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
/// 定向众筹结算（0078，HDBits Featured 口径）：
///   达标（raised ≥ goal）→ 挂 free×hours 促销 + status=1 + 通知发起人；
///   到期未达标 → 按实付全额退款（含税，税由池子承担）+ status=2。
/// 幂等：状态位先行（UPDATE ... WHERE status=0 RETURNING），重跑无副作用。
async fn funding_settle(db: &PgPool) -> anyhow::Result<u64> {
    // ① 达标
    let reached: Vec<(i64, i64, i32)> = sqlx::query_as(
        r#"
        UPDATE fundings SET status = 1, promoted_at = now()
        WHERE status = 0 AND raised >= goal
        RETURNING id, torrent_id, hours
        "#,
    )
    .fetch_all(db)
    .await?;
    for (fid, tid, hours) in &reached {
        sqlx::query(
            "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source) \
             VALUES ('torrent', $1, 'free', now(), now() + make_interval(hours => $2::int), 'task') \
             ON CONFLICT DO NOTHING",
        )
        .bind(tid)
        .bind(*hours as i64)
        .execute(db)
        .await?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             SELECT NULL, creator_id, '众筹达标', \
                    '种子 #' || $1 || ' 的众筹已达标，已挂 ' || $2 || ' 小时免费促销。' \
             FROM fundings WHERE id = $3",
        )
        .bind(tid)
        .bind(*hours as i64)
        .bind(fid)
        .execute(db)
        .await;
    }
    // ② 到期未达标 → 退款。审计修复（P1 撕裂）：旧版先置 status=2 再退款，进程在两步间
    // 死掉后该众筹永久退出结算集合（WHERE status=0 匹配不到），未完成的退款丢失。
    // 新序：CAS 到中间态 3（结算中）→ 逐笔退款 → 全部成功置 2；重启后 3 态重入续退。
    let mut expired: Vec<(i64, i64)> = sqlx::query_as(
        "UPDATE fundings SET status = 3 WHERE status = 0 AND ends_at <= now() RETURNING id, creator_id",
    )
    .fetch_all(db)
    .await?;
    // 上次崩溃残留的 3 态（结算中）重新纳入本轮退款
    expired.extend(
        sqlx::query_as::<_, (i64, i64)>("SELECT id, creator_id FROM fundings WHERE status = 3")
            .fetch_all(db)
            .await?,
    );
    let mut refunds = 0u64;
    for (fid, _creator) in &expired {
        let contribs: Vec<(i64, i64)> =
            sqlx::query_as("SELECT user_id, amount FROM funding_contribs WHERE funding_id = $1")
                .bind(fid)
                .fetch_all(db)
                .await?;
        for (uid, amount) in &contribs {
            let idem = format!("funding-refund:{fid}:{uid}");
            sqlx::query(
                r#"
                INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
                SELECT nextval('spark_ledger_id_seq'), $1, $2, 'funding_refund', $3
                WHERE NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)
                "#,
            )
            .bind(uid)
            .bind(amount)
            .bind(&idem)
            .execute(db)
            .await?;
            sqlx::query("UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 AND NOT EXISTS (SELECT 1 FROM spark_ledger WHERE idempotency_key = $3)")
                .bind(uid)
                .bind(amount)
                .bind(&idem)
                .execute(db)
                .await?;
            refunds += 1;
        }
        // 全部退款成功 → 终态 2（失败时下一轮从 3 态重入续退，幂等键防双退）
        sqlx::query("UPDATE fundings SET status = 2 WHERE id = $1")
            .bind(fid)
            .execute(db)
            .await?;
        let _ = sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) \
             SELECT NULL, creator_id, '众筹未达标', \
                    '种子相关众筹到期未达标，参与者的魔力已全额退款（含赠送税部分）。' \
             FROM fundings WHERE id = $1",
        )
        .bind(fid)
        .execute(db)
        .await;
    }
    if !reached.is_empty() || !expired.is_empty() {
        tracing::info!(
            reached = reached.len(),
            expired = expired.len(),
            refunds,
            "funding_settle"
        );
    }
    Ok((reached.len() + expired.len()) as u64)
}

/// 泄露者检测（0078，U3D LeakerController 离线简化版）：
///   ① passkey 多地并发：7 天内同一用户下载行为来自 ≥3 个不同公网 IP 且跨运营商
///      级网段（/16 不同）→ 疑似 passkey 泄露；
///   ② 首发抢发：种子过审后 10 分钟内即出现完成下载的非常规 agent。
/// 数据源 login_events（已有 IP 记录）+ traffic_ledger。只写 leak_events 供 staff 复核，不自动处罚。
async fn leak_scan(db: &PgPool) -> anyhow::Result<u64> {
    // ① passkey 多段 IP（登录侧证据）：7 天内成功登录跨 ≥3 个 /16（v6 为 /32）网段
    let susp1: Vec<(i64, i16, serde_json::Value)> = sqlx::query_as(
        r#"
        WITH grouped AS (
          SELECT user_id,
                 count(DISTINCT CASE WHEN family(ip) = 4 THEN set_masklen(ip, 16) END) AS v4_segs,
                 count(DISTINCT CASE WHEN family(ip) = 6 THEN set_masklen(ip, 32) END) AS v6_segs,
                 count(DISTINCT ip) AS ips
          FROM login_events
          WHERE ok AND ip IS NOT NULL AND created_at > now() - interval '7 days'
          GROUP BY user_id
        )
        SELECT user_id,
               LEAST(100, 40 + 15 * GREATEST(v4_segs, v6_segs))::smallint AS score,
               jsonb_build_object('ips', ips, 'v4_segs', v4_segs, 'v6_segs', v6_segs) AS detail
        FROM grouped
        WHERE GREATEST(v4_segs, v6_segs) >= 3
          AND NOT EXISTS (SELECT 1 FROM users u WHERE u.id = grouped.user_id AND u.class_id >= 90)
        "#,
    )
    .fetch_all(db)
    .await?;
    // ② 首发快速完成（下载侧证据）：过审 10 分钟内即完成下载（抢发/泄露的典型特征）
    let susp2: Vec<(i64, i16, serde_json::Value)> = sqlx::query_as(
        r#"
        SELECT s.user_id, 60::smallint,
               jsonb_build_object('torrent_id', s.torrent_id,
                                  'completed_at', s.completed_at,
                                  'approved_at', t.approved_at,
                                  'downloaded', s.downloaded)
        FROM snatches s JOIN torrents t ON t.id = s.torrent_id
        WHERE s.completed_at IS NOT NULL
          AND t.approved_at IS NOT NULL
          AND s.completed_at < t.approved_at + interval '10 minutes'
          AND t.approved_at > now() - interval '7 days'
          AND NOT EXISTS (SELECT 1 FROM users u WHERE u.id = s.user_id AND u.class_id >= 90)
        "#,
    )
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    // 两类来源分开写（kind 语义清晰），去重键 (user, kind, torrent)×3 天窗
    for (uid, score, detail) in susp1.iter() {
        let r = sqlx::query(
            r#"
            INSERT INTO leak_events (kind, user_id, torrent_id, detail, score)
            SELECT 'passkey_multi_ip', $1, NULL, $2, $3
            WHERE NOT EXISTS (
                SELECT 1 FROM leak_events e
                WHERE e.user_id = $1 AND e.kind = 'passkey_multi_ip'
                  AND e.created_at > now() - interval '3 days'
            )
            "#,
        )
        .bind(uid)
        .bind(detail)
        .bind(*score)
        .execute(db)
        .await?;
        n += r.rows_affected();
    }
    for (uid, score, detail) in susp2.iter() {
        let tid = detail.get("torrent_id").and_then(|v| v.as_i64());
        let r = sqlx::query(
            r#"
            INSERT INTO leak_events (kind, user_id, torrent_id, detail, score)
            SELECT 'first_dl_fast', $1, $2, $3, $4
            WHERE NOT EXISTS (
                SELECT 1 FROM leak_events e
                WHERE e.user_id = $1 AND e.kind = 'first_dl_fast'
                  AND (e.torrent_id IS NOT DISTINCT FROM $2)
                  AND e.created_at > now() - interval '3 days'
            )
            "#,
        )
        .bind(uid)
        .bind(tid)
        .bind(detail)
        .bind(*score)
        .execute(db)
        .await?;
        n += r.rows_affected();
    }
    if n > 0 {
        tracing::warn!(n, "leak_scan: 可疑事件写入 leak_events，待 staff 复核");
    }
    Ok(n)
}

/// Refundable 结算（0078，U3D 口径：边做种边退下载量）：
/// 促销档 kind='refundable' 生效期间，参与者下载量按「做种时长线性退还」——
/// 每 24h 做种退该种计费下载的 1/14（两周退满，与 H&R 120h 宽限同量级）。
/// 退款以负流量流水落 traffic_ledger（delta_down 为负），users 快照由 reconcile 聚合。
/// 幂等：当日已退过（同用户同种当日存在负流水）不再退——一天一结。
async fn refundable_settle(db: &PgPool) -> anyhow::Result<u64> {
    let res = sqlx::query(
        r#"
        WITH eligible AS (
            SELECT s.user_id, s.torrent_id, s.downloaded
            FROM snatches s
            JOIN promotions p ON p.torrent_id = s.torrent_id
              AND p.kind = 'refundable' AND p.starts_at <= now() AND p.ends_at > now()
            WHERE s.downloaded > 0 AND s.seeded_seconds >= 86400
        ),
        due AS (
            SELECT user_id, torrent_id, downloaded,
                   -LEAST(downloaded, downloaded / 14) AS refund
            FROM eligible
        )
        INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start)
        SELECT nextval('traffic_ledger_id_seq'), user_id, torrent_id, 0, refund, now()
        FROM due d
        WHERE NOT EXISTS (
            SELECT 1 FROM traffic_ledger l
            WHERE l.user_id = d.user_id AND l.torrent_id = d.torrent_id
              AND l.delta_down < 0 AND l.window_start > current_date
        )
        "#,
    )
    .execute(db)
    .await?;
    if res.rows_affected() > 0 {
        tracing::info!(n = res.rows_affected(), "refundable_settle: 下载量退还落账");
    }
    Ok(res.rows_affected())
}

pub async fn run_all(db: PgPool, redis: redis::aio::ConnectionManager) -> anyhow::Result<()> {
    let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
    let mut hour_tick = tokio::time::interval(std::time::Duration::from_secs(3600));
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
                // 审计修复（多实例互斥 + 超时）：每个 job 包 advisory lock + 900s 超时。
                // 多 worker 部署时同 job 只有抢到锁的实例执行（拿不到锁静默跳过本轮）；
                // 卡死任务 15 分钟后被 timeout 掐掉、连接归还，不会拖垮整个调度循环。
                // 失败/超时在 with_lock 内统一记日志（含 key），此处无需再逐个 match。
                with_lock(&db, "job:expire_promotions", expire_promotions(&db)).await;
                with_lock(&db, "job:magic_pool_promo", magic_pool_promo(&db)).await;
                with_lock(&db, "job:preserve_exit", preserve_exit(&db)).await;
                // consume_* 依赖 Redis 游标，天然单游标推进；但多实例并发拉同一段流
                // 仍会双计——同样入锁。ConnectionManager 是 clone 句柄，clone 后移入闭包；
                // PgPool 同样 clone（Arc 池句柄，代价可忽略），避免与外层 &db 借用冲突。
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    with_lock(&db, "job:consume_announce", async move {
                        consume_announce(&db2, &mut r).await
                    })
                    .await;
                }
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    with_lock(&db, "job:consume_agent_blocks", async move {
                        consume_agent_blocks(&db2, &mut r).await
                    })
                    .await;
                }
                with_lock(&db, "job:backfill_pieces_hash", backfill_pieces_hash(&db)).await;
                with_lock(&db, "job:sweep_stale_peers", sweep_stale_peers(&db)).await;
                with_lock(&db, "job:collect_milestones", collect_milestones(&db)).await;
                with_lock(&db, "job:hr_enforce", hr_enforce(&db)).await;
                with_lock(&db, "job:hr_punish", hr_punish(&db)).await;
                with_lock(&db, "job:class_auto_adjust", class_auto_adjust(&db)).await;
                with_lock(&db, "job:preserve_seed", preserve_seed(&db)).await;
                with_lock(&db, "job:task_settle", crate::task_jobs::task_settle(&db)).await;
                with_lock(&db, "job:exam_assign", crate::task_jobs::exam_assign(&db)).await;
                // 论坛抽奖到点开奖（0126）：draw_at 已过且仍 open 的逐个开。
                // 开奖逻辑（CAS open→drawn + 按人幂等发放）在 sqlx 层面自守，这里独立
                // 实现一份轻量扫描（worker 不依赖 api crate），锁内重跑安全。
                {
                    let due: Vec<i64> = sqlx::query_scalar(
                        "SELECT topic_id FROM topic_lotteries WHERE status = 'open' AND draw_at <= now() LIMIT 50",
                    )
                    .fetch_all(&db)
                    .await
                    .unwrap_or_default();
                    for tid in due {
                        if let Err(e) = lottery_settle(&db, tid).await {
                            tracing::warn!(topic_id = tid, error = %e, "lottery_settle failed");
                        }
                    }
                }
                // 银行结算：站点时区 UTC+8 自然日切换后跑一次；分钟级检查保证 worker 重启/宕机跨日也能补跑
                let site_day = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
                if last_bank_day.is_none() {
                    last_bank_day = Some(init_bank_day(&db).await);
                }
                if last_bank_day != Some(site_day) {
                    tracing::info!(?site_day, "bank_daily start");
                    // bank_daily 内部各子步骤自带日期游标/幂等键，锁内重跑安全
                    let db2 = db.clone();
                    with_lock(&db, "job:bank_daily", async move {
                        crate::bank_jobs::bank_daily(&db2).await;
                        Ok::<(), anyhow::Error>(())
                    })
                    .await;
                    // 结算失败（游标未写）时保持 last_bank_day 落后，下一分钟 tick 重试整轮；
                    // 成功时以 bank_settle_runs 的 run_date 为准，避免与库内游标漂移。
                    let after: Option<chrono::NaiveDate> =
                        sqlx::query_scalar("SELECT max(run_date) FROM bank_settle_runs")
                            .fetch_one(&db)
                            .await
                            .ok()
                            .flatten();
                    last_bank_day = Some(after.unwrap_or(site_day - chrono::Duration::days(1)));
                }
            }
            _ = hour_tick.tick() => {
                // 审计修复：去掉 first_hour 首轮跳过——原逻辑为防启动风暴，但 worker
                // 频繁重启（崩溃循环/滚动发布）时 hour_interval 每次都从第一 tick 起步，
                // seeding_reward 可能数小时不被执行。本任务幂等键 = seeding:{user}:{yyyymmddhh}，
                // 同小时重复执行零副作用；其余 hourly 任务也都自带幂等护栏，首轮直接跑安全。
                // 时魔参数（底薪/封顶/曲线/体积基准/稀有度/标定）全在 site_settings `seeding_*`，
                // 由 DB 函数 seeding_params() 统一读取 —— 调价不再改代码重发（迁移 0133）。
                // 结算前先拿僵尸阈值传给结算 SQL（同 tick 内不依赖 sweep_stale_peers 是否跑过）
                let stale_secs = stale_peer_threshold_secs(&db).await;
                with_lock(&db, "job:seeding_reward", seeding_reward(&db, stale_secs)).await;
                with_lock(&db, "job:purge_old_login_events", purge_old_login_events(&db)).await;
                with_lock(&db, "job:ratio_watch", ratio_watch(&db)).await;
                with_lock(&db, "job:dormant_mark", dormant_mark(&db)).await;
                with_lock(&db, "job:wishlist_notify", wishlist_notify(&db)).await;
                with_lock(&db, "job:highspeed_tag", highspeed_tag(&db)).await;
                with_lock(&db, "job:resurrection_settle", resurrection_settle(&db)).await;
                with_lock(&db, "job:social_team_settle", social_team_settle(&db)).await;
                // 失败判定必须排在成功结算之后：恰好在期限内达标的队伍应算成功
                with_lock(&db, "job:social_team_expire", social_team_expire(&db)).await;
                // 绩效考核月末结算（0106）：挂 hourly 而非 daily——daily tick 首轮被
                // first_tick1d 跳过、重启后要等 24h 才首跑；hourly 首轮立即执行，
                // 且本 job 幂等（settled_at 标记 + 发薪幂等键），空扫描是一次索引查询
                with_lock(&db, "job:jixiao_settle", jixiao_settle(&db)).await;
                with_lock(&db, "job:preserve_settle", preserve_settle(&db)).await;
                with_lock(&db, "job:funding_settle", funding_settle(&db)).await;
                with_lock(&db, "job:refundable_settle", refundable_settle(&db)).await;
                with_lock(&db, "job:achievement_grant", achievement_grant(&db)).await;
                // 卫生清理（NP docleanup 口径）：过期邀请落库回收 / 一次性凭证与重置 token 清理
                with_lock(&db, "job:expire_invites", expire_invites(&db)).await;
                with_lock(&db, "job:purge_expired_tokens", purge_expired_tokens(&db)).await;
                // DLQ 可见性：只进不出等于变相丢计费——有积压时通知管理组信箱
                {
                    let (db2, mut r) = (db.clone(), redis.clone());
                    with_lock(&db, "job:dlq_watch", async move {
                        dlq_watch(&db2, &mut r).await
                    })
                    .await;
                }
            }
            _ = tick10.tick() => {
                if first_tick10 { first_tick10 = false; continue; }
                with_lock(&db, "job:cheat_audit", cheat_audit(&db)).await;
            }
            _ = tick30.tick() => {
                if first_tick30 { first_tick30 = false; continue; }
                with_lock(&db, "job:multi_ip_check", multi_ip_check(&db)).await;
                with_lock(&db, "job:leak_scan", leak_scan(&db)).await;
            }
            _ = tick6h.tick() => {
                if first_tick6h { first_tick6h = false; continue; }
                // 对账告警必须先于 reconcile_snapshots：收敛会抹掉差异证据
                with_lock(&db, "job:reconcile_diff_alert", reconcile_diff_alert(&db)).await;
                with_lock(&db, "job:reconcile_snapshots", reconcile_snapshots(&db)).await;
            }
            _ = tick1d.tick() => {
                if first_tick1d { first_tick1d = false; continue; }
                with_lock(&db, "job:ensure_partitions", ensure_partitions(&db)).await;
            }
        }
    }
}

/// 多实例互斥 + per-job 超时（审计修复）。
/// - 互斥：pg_try_advisory_lock(hashtext(key)) 拿不到（他实例在跑）→ 返回 None 静默跳过本轮；
/// - 超时：tokio::time::timeout 900s 掐掉卡死任务（超时按失败上报）；
/// - 连接口径：advisory lock 是会话级，lock/unlock 必须落在同一条连接上——
///   从 pool acquire 一条专用连接持锁，业务 future 用整个 pool（不占锁连接），
///   完成后在同一连接 unlock。业务超时被掐后 unlock 仍执行，锁不残留。
/// 论坛抽奖开奖（0126，worker 侧）：与 api 的 lottery_draw_core 同一套库表协议——
/// CAS open→drawn 防双开，中奖发放幂等键 `forum-lottery-win:{tid}:{uid}`（spark_ledger 自守），
/// 无人参与退回楼主（`forum-lottery-refund:{tid}`）。票费不分成（归入池的是楼主冻结的奖金，
/// 票费在本实现里是参与门槛而非奖池构成，避免开奖金额与冻结额错位）。
async fn lottery_settle(db: &PgPool, topic_id: i64) -> anyhow::Result<u64> {
    let meta: Option<(i32, i64, i64)> = sqlx::query_as(
        "SELECT winners, prize_per_winner, ticket_spark::bigint FROM topic_lotteries \
         WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(topic_id)
    .fetch_optional(db)
    .await?;
    let Some((winners, prize, _ticket)) = meta else {
        return Ok(0); // 已开/已取消：幂等静默
    };
    let n = sqlx::query(
        "UPDATE topic_lotteries SET status = 'drawn' WHERE topic_id = $1 AND status = 'open'",
    )
    .bind(topic_id)
    .execute(db)
    .await?
    .rows_affected();
    if n == 0 {
        return Ok(0); // 并发对手（楼主手动开）赢了对局
    }
    let mut tx = db.begin().await?;
    // 中奖名单：数据库侧 random() 洗牌取前 N（抽签随机性不由应用层承担）
    let picked: Vec<i64> = sqlx::query_scalar(
        "UPDATE lottery_entries SET won = TRUE \
         WHERE topic_id = $1 AND user_id IN ( \
           SELECT user_id FROM lottery_entries WHERE topic_id = $1 ORDER BY random() LIMIT $2 \
         ) RETURNING user_id",
    )
    .bind(topic_id)
    .bind(winners)
    .fetch_all(&mut *tx)
    .await?;
    if picked.is_empty() {
        // 无人参与：奖金池整退楼主
        let op: i64 = sqlx::query_scalar("SELECT user_id FROM topics WHERE id = $1")
            .bind(topic_id)
            .fetch_one(&mut *tx)
            .await?;
        let refund = winners as i64 * prize;
        if refund > 0 {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
            )
            .bind(format!("forum-lottery-refund:{topic_id}"))
            .fetch_one(&mut *tx)
            .await?;
            if !exists {
                let bal: i64 = sqlx::query_scalar(
                    "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 RETURNING spark_balance",
                )
                .bind(op)
                .bind(refund)
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query(
                    "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
                     VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'forum_lottery_refund', $3, $4)",
                )
                .bind(op)
                .bind(refund)
                .bind(format!("forum-lottery-refund:{topic_id}"))
                .bind(bal)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        tracing::info!(topic_id, refund, "lottery settled: no entries, refunded");
        return Ok(0);
    }
    // 发放（同事务逐人：幂等键存在则跳过，重跑安全）
    for uid in &picked {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE idempotency_key = $1)",
        )
        .bind(format!("forum-lottery-win:{topic_id}:{uid}"))
        .fetch_one(&mut *tx)
        .await?;
        if exists || prize <= 0 {
            continue;
        }
        let bal: i64 = sqlx::query_scalar(
            "UPDATE users SET spark_balance = spark_balance + $2 WHERE id = $1 RETURNING spark_balance",
        )
        .bind(uid)
        .bind(prize)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key, balance_after) \
             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'forum_lottery', $3, $4)",
        )
        .bind(uid)
        .bind(prize)
        .bind(format!("forum-lottery-win:{topic_id}:{uid}"))
        .bind(bal)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    tracing::info!(topic_id, winners = picked.len(), prize, "lottery settled");
    Ok(picked.len() as u64)
}

async fn with_lock<F, T>(db: &PgPool, key: &str, fut: F) -> Option<T>
where
    F: std::future::Future<Output = anyhow::Result<T>>,
{
    // U1 §5.4 模块守卫：job 声明归属模块则按开关整轮跳过（debug 日志，不动账）；
    // 核心任务（announce 计费/快照/清理/反作弊）不在表内 = 不受开关影响。
    // 跳过不报错，恢复开启后靠既有幂等键自然补跑。
    if let Some(module) = job_module(key) {
        if !module_on(db, module).await {
            tracing::debug!(key, module, "module off, skip job");
            return None;
        }
    }
    let mut conn = match db.acquire().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(?e, key, "advisory lock 连接获取失败，跳过本轮");
            return None;
        }
    };
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock(hashtext($1))")
        .bind(key)
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(false);
    if !locked {
        tracing::debug!(key, "advisory lock 未抢到（他实例执行中），跳过本轮");
        return None;
    }
    // 业务 future 与锁连接解耦：超时只掐业务，不掐持锁连接
    let outcome = tokio::time::timeout(std::time::Duration::from_secs(900), fut).await;
    // 同一连接上解锁（连接归还池前必须释放，否则锁随连接泄漏到复用方）
    let unlock: Result<bool, _> = sqlx::query_scalar("SELECT pg_advisory_unlock(hashtext($1))")
        .bind(key)
        .fetch_one(&mut *conn)
        .await;
    if let Err(e) = unlock {
        tracing::error!(?e, key, "advisory unlock 失败（锁将随连接关闭释放）");
    }
    match outcome {
        Ok(Ok(v)) => Some(v),
        Ok(Err(e)) => {
            tracing::error!(?e, key, "job 执行失败");
            None
        }
        Err(_) => {
            tracing::error!(key, "job 超时（900s）被掐断");
            None
        }
    }
}

/// U1 §5.4：job key → 模块键映射（与 API 网关表同口径）。
/// 未列出的 job 属核心层（计费/快照/清理/反作弊/等级），不受模块开关影响。
fn job_module(job_key: &str) -> Option<&'static str> {
    Some(match job_key {
        "job:bank_daily" => "bank",
        "job:task_settle" => "tasks",
        "job:exam_assign" => "exams",
        "job:jixiao_settle" => "jixiao",
        "job:social_team_settle" | "job:social_team_expire" => "social",
        "job:preserve_exit" | "job:preserve_settle" | "job:preserve_seed" => "preserve",
        "job:resurrection_settle" => "resurrections",
        "job:wishlist_notify" => "wishlist",
        _ => return None,
    })
}

/// 模块开关判定（worker 侧直查，无缓存——每分钟 tick 一次，查询代价可忽略；
/// 与 API 的 ModuleFlags::default_on 保持同一缺省口径：缺键=教育站形态）。
async fn module_on(db: &PgPool, module: &str) -> bool {
    let v: Option<String> = sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
        .bind(format!("module_{module}"))
        .fetch_optional(db)
        .await
        .unwrap_or(None);
    match v {
        // 显式配置按配置（no = 关）；查询失败/未配置回落默认值（T3 缺省=现状）
        Some(raw) => raw.trim() == "yes",
        None => !matches!(module, "showcase" | "social" | "contests"),
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

    /// 做种收益公式口径锁定：与迁移 0133 的 DB 函数（`seeding_torrent_bonus` / `seeding_hourly`）
    /// 同构的 Rust 镜像。**公式的唯一权威在 DB**（worker 结算与 API 预估都调它），
    /// 本镜像用于快速单测；两者一致性由 `_v_seeding.py` 用 180 组参数逐组对账（1e-9 容差）守住。
    /// 常量必须与迁移 0133 的 site_settings 缺省值一致（vol_base 50 / rarity 0.6,0.35 /
    /// scale 0.4 / cap 150 / curve_k 0.12）。
    const VOL_BASE: f64 = 50.0;
    const RARITY_K: f64 = 0.6;
    const RARITY_EXP: f64 = 0.35;
    const SCALE: f64 = 0.4;
    const CAP: f64 = 150.0;
    const CURVE_K: f64 = 0.12;
    const GIB: f64 = 1073741824.0;

    /// 种子维度档位分（第一命中优先）
    fn rule_seed(size: i64, seeders: i64, age_days: f64, completed: i64) -> f64 {
        if seeders <= 1 && completed >= 3 {
            2.0
        } else if age_days > 365.0 {
            1.5
        } else if age_days > 180.0 {
            1.0
        } else if size as f64 >= 100.0 * GIB {
            0.75
        } else if size as f64 >= 25.0 * GIB {
            0.5
        } else {
            0.25
        }
    }

    /// 个人做种时长档位分（奖励长期保种，替代旧的 1/(1+h/2160) 衰减）
    fn dur_bonus(personal_hours: f64) -> f64 {
        if personal_hours >= 8760.0 {
            2.0
        } else if personal_hours >= 4320.0 {
            1.0
        } else if personal_hours >= 2160.0 {
            0.75
        } else if personal_hours >= 720.0 {
            0.5
        } else {
            0.0
        }
    }

    fn torrent_bonus(
        size: i64,
        seeders: i64,
        age_days: f64,
        completed: i64,
        personal_hours: f64,
    ) -> f64 {
        // 与 DB 一致地用自然对数（比值等价，避免 log10/ln 混用造成对账口径分歧）
        let vol = ((1.0 + size as f64 / GIB).ln() / (1.0 + VOL_BASE).ln()).min(1.0);
        let rar = 1.0 + RARITY_K * (seeders.max(1) as f64).powf(-RARITY_EXP);
        (rule_seed(size, seeders, age_days, completed) + dur_bonus(personal_hours))
            * vol
            * rar
            * SCALE
    }

    /// (size, seeders, age_days, completed, personal_hours) → 每小时魔力（不含 donor 倍数）
    fn user_hourly(base: f64, torrents: &[(i64, i64, f64, i64, f64)]) -> f64 {
        let sum: f64 = torrents
            .iter()
            .map(|(z, s, a, c, h)| torrent_bonus(*z, *s, *a, *c, *h))
            .sum();
        base + (2.0 / std::f64::consts::PI * CAP * (sum * CURVE_K).atan()).floor()
    }

    #[test]
    fn seeding_formula_volume_monotonic_and_saturated() {
        // 体积单调且在对数基准（50GB）处饱和
        let g1 = torrent_bonus(1 * GIB as i64, 5, 60.0, 10, 100.0);
        let g10 = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 100.0);
        let g100 = torrent_bonus(100 * GIB as i64, 5, 60.0, 10, 100.0);
        let t1 = torrent_bonus(1024 * GIB as i64, 5, 60.0, 10, 100.0);
        assert!(g1 < g10 && g10 < g100, "{g1} {g10} {g100}");
        assert!((g100 - t1).abs() < 1e-12, "体积应饱和: {g100} vs {t1}");
    }

    #[test]
    fn seeding_small_torrent_pile_is_not_profitable() {
        // 本次改造的核心：堆 1000 颗 1MB 小种只能拿底薪（旧公式可拿到 ~205/h）
        let pile: Vec<(i64, i64, f64, i64, f64)> = vec![(1048576, 1, 3.0, 0, 1.0); 1000];
        let pile_hourly = user_hourly(10.0, &pile);
        let honest: Vec<(i64, i64, f64, i64, f64)> =
            vec![(100 * GIB as i64, 1, 400.0, 5, 8760.0); 6];
        let honest_hourly = user_hourly(10.0, &honest);
        assert!(pile_hourly <= 12.0, "堆小种不应超出底薪: {pile_hourly}");
        assert!(honest_hourly >= 80.0, "老实保种应拿得多: {honest_hourly}");
        assert!(
            honest_hourly >= pile_hourly * 8.0,
            "{honest_hourly} vs {pile_hourly}"
        );
    }

    #[test]
    fn seeding_personal_time_rewards_loyalty() {
        // 与旧公式相反的故意改动：挂得越久收益越高（对齐 Gazelle/U3D 的长期保种激励）
        let short = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 240.0);
        let mid = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 2400.0);
        let year = torrent_bonus(10 * GIB as i64, 5, 60.0, 10, 8760.0);
        assert!(short < mid && mid < year, "{short} {mid} {year}");
    }

    #[test]
    fn seeding_rarity_bonus_not_penalty() {
        // 档位隔离（全是"日常种"）：独苗最高，人多趋近 1 而不是趋近 0
        let lone = torrent_bonus(10 * GIB as i64, 1, 60.0, 0, 100.0);
        let ten = torrent_bonus(10 * GIB as i64, 10, 60.0, 0, 100.0);
        let hundred = torrent_bonus(10 * GIB as i64, 100, 60.0, 0, 100.0);
        assert!(lone > ten && ten > hundred, "{lone} {ten} {hundred}");
        // 独苗/10 人 = rarity(1)/rarity(10) = 1.6 / 1.2679
        assert!((lone / ten - 1.6 / (1.0 + 0.6 * 10.0_f64.powf(-0.35))).abs() < 1e-9);
        // 热门不再被重罚：100 人时仍保留 5 人时的 80% 以上（旧公式 ≈35%）
        let five = torrent_bonus(10 * GIB as i64, 5, 60.0, 0, 100.0);
        assert!(hundred / five > 0.80, "{}", hundred / five);
    }

    #[test]
    fn seeding_formula_arctan_cap() {
        // 1000 颗濒危种也只在软封顶内（渐近 base+150，且 atan 开区间取不到）
        let many: Vec<(i64, i64, f64, i64, f64)> =
            vec![(100 * GIB as i64, 1, 400.0, 5, 8760.0); 1000];
        let total = user_hourly(10.0, &many);
        assert!(total < 10.0 + CAP, "total={total}");
        assert!(total > 10.0 + 100.0, "total={total}（应明显超过半程）");
    }

    #[test]
    fn seeding_formula_dying_beats_daily() {
        // 濒危保种（档位 2.0）时薪远高于日常种（0.25）——激励方向正确。
        // 同为 10GB 种子以隔离体积因子：比值应为 2.0/0.25 = 8
        let dying = torrent_bonus(10 * GIB as i64, 1, 400.0, 5, 100.0);
        let daily = torrent_bonus(10 * GIB as i64, 1, 10.0, 1, 100.0);
        assert!(dying > daily * 7.0, "dying={dying} daily={daily}");
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
