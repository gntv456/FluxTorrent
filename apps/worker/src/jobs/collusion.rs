//! 对刷与谎报检测（2026-10-07 假种/作弊审计缺口三件套，迁移 0301）。
//!
//! 三类只在「数据交叉」后才现形的作弊（单看任何一张表都正常）：
//!   ① self_deal —— 同 IP 双账号对刷：A 账号做种、B 账号下载，账面全真、
//!      实为一人对倒。信号 = 同一 IP 在同一 swarm 同时出现两个账号且
//!      一个在传一个在下（NAT 场景双方同角色，不命中，误伤面小）。
//!   ② down_under —— 谎报下载量：各 seeder 的 xreport 佐证了「swarm 总共
//!      传给该 leecher N 字节」，其自报 downloaded 却远小于 N —— 魔改
//!      客户端压低下载拉高 ratio 的画像特征。
//!   ③ bitthief —— 完成后零上传长期挂机：连通端口正常、swarm 一直有人
//!      等数据，却从未上传过一个字节（乐观解锁机制的纯索取者）。
//!   ④ near_cap —— 贴边汇报节奏：网盘临时挂载型假做种（0304），间隔
//!      长期贴着容忍上限是它抹不掉的指纹（正常客户端按 interval 汇报）。
//! 四者都只落 cheat_events（进管理组待办处置视图），不自动处罚——
//! 教育站误伤代价高于放行（NAT 型恶劣环境的真实用户可能命中 ②/③）。

use sqlx::PgPool;

/// 四合一入口（run.rs 调度 + 面板手触）：①对刷 ②谎报下载 ③BitThief
/// ④贴边汇报，顺带做两张窗口日志表的裁剪。返回本轮新增 cheat_events 数。
pub async fn collusion_check(db: &PgPool) -> anyhow::Result<u64> {
    let deals = self_deal_check(db).await?;
    let unders = down_under_check(db).await?;
    let thieves = bitthief_check(db).await?;
    let near_caps = near_cap_check(db).await?;
    let _ = prune_announce_ips(db).await;
    let _ = prune_leecher_xreports(db).await;
    let total = deals + unders + thieves + near_caps;
    if total > 0 {
        tracing::warn!(
            deals,
            unders,
            thieves,
            near_caps,
            "collusion_check 命中"
        );
    }
    Ok(total)
}

/// ④ 贴边汇报节奏（0304）：网盘挂 NAS 挂机型假做种专杀。
/// 手法 = 平时撤存储、只在汇报前临时挂上——客户端常开所以回连探测
/// 测不出数据不在盘（端口应答正常），唯一抹不掉的指纹是 announce
/// 间隔长期贴着做种时长容忍上限（seed_cap=2×interval）之下：挂
/// ~3400-3590s 汇报一次、每轮照拿满间隔时长。正常客户端按 interval
/// （=cap/2）汇报，抖动/休眠只是偶发贴边。
/// 画像：计入时长的汇报 ≥20 次且贴边占比 ≥80%。只记录不处罚——
/// 弱网/移动端用户可能长期高间隔（仍是真实挂种），留人工裁量。
pub async fn near_cap_check(db: &PgPool) -> anyhow::Result<u64> {
    // 计数列是 integer（INT4）——解码按 i32，防 sqlx 类型协商失败
    let rows: Vec<(i64, i64, i32, i32)> = sqlx::query_as(
        r#"
        SELECT s.user_id, s.torrent_id,
               s.near_cap_announces, s.total_announces
        FROM snatches s
        WHERE s.total_announces >= 20
          AND s.near_cap_announces * 100 >= s.total_announces * 80
          AND s.last_seen_at > now() - interval '2 days'
        "#,
    )
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for (uid, torrent_id, near, total) in rows {
        let _ = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, reason) \
             VALUES ($1, $2, \
             'near_cap_rhythm（汇报间隔长期贴近容忍上限，疑似临时挂载型假做种）') \
             ON CONFLICT (user_id, agent, reason) DO UPDATE \
               SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(uid)
        .bind(format!("nearcap:{torrent_id}"))
        .execute(db)
        .await?;
        tracing::warn!(
            user = uid,
            torrent = torrent_id,
            near,
            total,
            "贴边汇报占比 ≥80%"
        );
        n += 1;
    }
    Ok(n)
}

/// ① 同 IP 双账号对刷（30 分钟窗口聚合 announce_ips）。
/// NAT 误伤规避：只认「同 IP 同 swarm 且一方 seeding、另一方 leeching」
/// 的方向性对倒；双方都在下载（NAT 拼团）或都在做种（同盒子保种）不命中。
pub async fn self_deal_check(db: &PgPool) -> anyhow::Result<u64> {
    // 近 30 分钟内，同 IP 同 swarm 里「我下你种 + 你下我种」任一方向的账号对。
    // 去重键 = 无序账号对 + hash（a<b 规范化），防止一 swarm 多 peer 重复计数。
    // 六轮审计 P1-A：旧写法 `recent a JOIN recent b` 是 O(n²) 自连接——
    // 30 分钟窗口内同 (ip, hash) 的行数平方级膨胀（2 万活跃 peer 的站
    // ≈ 6.9 亿中间行，实测 56 行就产生 400 行过滤）。改为先按 (ip, hash)
    // 聚合成小数组再在数组内配对：每组通常只有 2~4 个账号，配对数是组内
    // 平方但组极小；聚合在索引扫描上下推，PG 只物化多账号组。
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        r#"
        WITH per_group AS (
            SELECT ip, info_hash,
                   array_agg(DISTINCT user_id)  AS users,
                   bool_or(seeding)              AS any_seeding,
                   bool_or(leeching)             AS any_leeching
            FROM announce_ips
            WHERE seen_at > now() - interval '30 minutes'
            GROUP BY ip, info_hash
            HAVING count(DISTINCT user_id) >= 2
               -- 方向性预筛：组内既出现过做种又出现过下载才可能配对
               AND bool_or(seeding) AND bool_or(leeching)
        ),
        deals AS (
            SELECT g.ip, g.info_hash,
                   LEAST(s1.user_id, s2.user_id)  AS ua,
                   GREATEST(s1.user_id, s2.user_id) AS ub
            FROM per_group g
            CROSS JOIN LATERAL unnest(g.users) AS s1(user_id)
            CROSS JOIN LATERAL unnest(g.users) AS s2(user_id)
            JOIN LATERAL (
                VALUES (s1.user_id, s2.user_id), (s2.user_id, s1.user_id)
            ) AS sides(seeder, leecher) ON true
            JOIN announce_ips a
              ON a.ip = g.ip AND a.info_hash = g.info_hash
             AND a.user_id = sides.seeder AND a.seeding
             AND a.seen_at > now() - interval '30 minutes'
            JOIN announce_ips b
              ON b.ip = g.ip AND b.info_hash = g.info_hash
             AND b.user_id = sides.leecher AND b.leeching
             AND b.seen_at > now() - interval '30 minutes'
            WHERE s1.user_id < s2.user_id
        )
        SELECT DISTINCT ua, ub, info_hash FROM deals
        "#,
    )
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for (ua, ub, hash) in rows {
        let agent = format!("deal:{}", &hash[..8.min(hash.len())]);
        for uid in [ua, ub] {
            let _ = sqlx::query(
                "INSERT INTO cheat_events (user_id, agent, peer_ip, reason) \
                 VALUES ($1, $2, $3, \
                 'self_deal（同 IP 双账号同 swarm 一传一下，疑似对刷）') \
                 ON CONFLICT (user_id, agent, reason) DO UPDATE \
                   SET hits = cheat_events.hits + 1, last_seen = now()",
            )
            .bind(uid)
            .bind(&agent)
            .bind(&hash)
            .execute(db)
            .await?;
        }
        n += 1;
    }
    Ok(n)
}

/// ② 谎报下载量：7 天窗口内 leecher 收到的佐证总量 vs 自报 downloaded。
/// 佐证来自各 seeder 的 xreport（按 leecher 汇总 leecher_xreports）。
/// 容差默认 20%（协议开销/未上报分片，site_settings 可调）；命中线 =
/// 佐证 ≥ 自报×（collusion_tolerance_ratio_pct/100）且差额 ≥ collusion_min_gap_gb。
/// 注意口径：自报 downloaded 是**累计值**而佐证只看 7 天窗口——窗口短于
/// 账龄时天然「佐证 < 自报」，不会误报；只有「窗口内佐证反超累计自报」
/// 才命中，即自报明显压低。
pub async fn down_under_check(db: &PgPool) -> anyhow::Result<u64> {
    // 六轮审计 P1-A：`sum(s.downloaded)` 是 NUMERIC，按 i64 解码会
    // "Rust type i64 is not compatible with SQL type NUMERIC"——本 job 自
    // 上线起每 10 分钟必炸（worker 日志实锤），谎报下载检测从未跑通。
    // 必须显式 ::bigint（sqlx 的 NUMERIC 老坑）。
    // 容差与最小命中差（0311 site_settings 化）：开源后出厂值公开，各站自调。
    // 容差以百分比整数存储（120 = 1.2x），SQL 侧乘除防浮点。
    let tolerance_pct: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings \
         WHERE name = 'collusion_tolerance_ratio_pct'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(120)
    .clamp(100, 500);
    let min_gap: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'collusion_min_gap_gb'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(1)
    .clamp(0, 1024)
        * 1024
        * 1024
        * 1024;
    let rows: Vec<(i64, i64, i64)> = sqlx::query_as(
        r#"
        SELECT l.leecher, l.corr_total, COALESCE(sum(s.downloaded), 0)::bigint
        FROM (
            SELECT leecher, sum(bytes)::bigint AS corr_total
            FROM leecher_xreports
            WHERE reported_at > now() - interval '7 days'
            GROUP BY leecher
        ) l
        JOIN snatches s ON s.user_id = l.leecher
        GROUP BY l.leecher, l.corr_total
        HAVING l.corr_total * 100 > COALESCE(sum(s.downloaded), 0)::bigint * $1
           AND l.corr_total - COALESCE(sum(s.downloaded), 0)::bigint
               > $2
        "#,
    )
    .bind(tolerance_pct)
    .bind(min_gap)
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for (uid, corr, self_reported) in rows {
        let _ = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, reason) \
             VALUES ($1, 'down_under', \
             'down_under（自报下载量低于 swarm 佐证量，疑似谎报下载）') \
             ON CONFLICT (user_id, agent, reason) DO UPDATE \
               SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(uid)
        .execute(db)
        .await?;
        tracing::warn!(
            user = uid,
            corroborated = corr,
            self_reported,
            "自报下载量显著低于佐证量"
        );
        n += 1;
    }
    Ok(n)
}

/// ③ BitThief 画像：completed ≥3 天、仍在种、零上传、端口正常，
/// 且该 swarm 近 7 天仍有 leecher 活跃（有人等数据却一毛不拔）。
/// 「无人下载的冷种做种者零上传」是正常保种，必须排除。
pub async fn bitthief_check(db: &PgPool) -> anyhow::Result<u64> {
    let rows: Vec<i64> = sqlx::query_scalar(
        r#"
        SELECT s.user_id
        FROM snatches s
        JOIN torrents t ON t.id = s.torrent_id
        WHERE s.seeding AND s.uploaded = 0
          AND s.completed_at IS NOT NULL
          AND s.completed_at < now() - interval '3 days'
          AND s.last_port > 0
          AND s.last_seen_at > now() - interval '2 days'
          AND EXISTS (
              SELECT 1 FROM snatches o
              WHERE o.torrent_id = s.torrent_id
                AND o.user_id <> s.user_id
                AND o.leeching
                AND o.last_seen_at > now() - interval '7 days'
          )
        "#,
    )
    .fetch_all(db)
    .await?;
    let mut n = 0u64;
    for uid in rows {
        let _ = sqlx::query(
            "INSERT INTO cheat_events (user_id, agent, reason) \
             VALUES ($1, 'bitthief', \
             'zero_upload_seeder（完成后长期零上传，疑似 BitThief/捂盘）') \
             ON CONFLICT (user_id, agent, reason) DO UPDATE \
               SET hits = cheat_events.hits + 1, last_seen = now()",
        )
        .bind(uid)
        .execute(db)
        .await?;
        n += 1;
    }
    Ok(n)
}

/// announce_ips 窗口裁剪（消费循环就地调用，不另开 job）：
/// 对刷检测只用 30 分钟窗口，留 2 小时余量供补跑/人工排查。
pub async fn prune_announce_ips(db: &PgPool) -> u64 {
    sqlx::query(
        "DELETE FROM announce_ips WHERE seen_at < now() - interval '2 hours'",
    )
    .execute(db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0)
}

/// leecher_xreports 窗口裁剪：对账窗口 7 天 + 1 天余量。
pub async fn prune_leecher_xreports(db: &PgPool) -> u64 {
    sqlx::query(
        "DELETE FROM leecher_xreports \
         WHERE reported_at < now() - interval '8 days'",
    )
    .execute(db)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0)
}
