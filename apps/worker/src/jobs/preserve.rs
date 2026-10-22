//! 保种结算。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

/// 保种区移出（M19 旧站口径：做种 > 阈值移出，免费延续 3 天）。
///
/// P2-8（2026-10-07）：出种人数阈值从硬编码 7 改为后台可配置
/// `uploader_seed_min`（默认 3，与「发布者保种出种 3 人」口径一致），
/// 站长可在设置面板「上传限制」卡调整。clamp 0..=50 防止填出
/// 永不移出（极大）或全部移出（0）的极端值；0 视为「不移出」。
pub async fn preserve_exit(db: &PgPool) -> anyhow::Result<u64> {
    let min_seeders: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'uploader_seed_min'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(3)
    .clamp(0, 50);
    // 阈值 0 = 关闭「做种人数达标即移出」，只保留人工移出
    if min_seeders == 0 {
        return Ok(0);
    }
    let res = sqlx::query(
        r#"
        WITH exited AS (
            UPDATE seed_preserve sp SET exited_at = now(), exit_reason = 'seeders_gt_threshold'
            FROM torrents t
            WHERE t.id = sp.torrent_id AND t.seeders > $1 AND sp.exited_at IS NULL
            RETURNING sp.torrent_id
        )
        INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source)
        SELECT 'torrent', torrent_id, 'free', now(), now() + interval '3 days', 'preserve_grace'
        FROM exited
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(min_seeders)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}

/// 保种认领结算（审计修复 P1 闭环补全）：claim 记了 seed_time_begin/uploaded_begin
/// 基线但无任何结算方——「认领→奖励」断裂为展示层。口径（NP claims 按时长发奖）：
///   每满 24h 有效保种（结算时点仍在做种）发 preserve_bonus_per_day（缺省 100，
///   site_settings 可调）；幂等键 = preserve:{torrent_id}:{user_id}:{认领轮次}:
///   {day_index}（轮次取自 claimed_at 的秒级纪元），重跑/补跑安全。停做种期间
///   不结算（恢复后在下个整天边界继续），时长基线列保留供后台展示 delta。
pub async fn preserve_settle(db: &PgPool) -> anyhow::Result<u64> {
    let bonus: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'preserve_bonus_per_day')::bigint, 100)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(100);
    let res = sqlx::query(
        r#"
        -- 三轮遗留（2026-10-07）：天数改为「已发次数 +1」而非挂钟差——旧口径
        -- 停种 N 天后回来，中间天数照发（只需结算时点在种）。新口径：
        -- 下一档 = 本认领轮次已结算的 preserve 行数 + 1，停种期间不产生新行；
        -- 仍要求结算时点在种 + 距上一档 ≥1 天（防反复上下线刷首档）。
        -- P0-2（2026-10-07 保种组审计）：「在种」补幽灵过滤——port>0 且回连
        -- 非「不可达」。实测 port=0 的纯 curl announce 即可令 seeding=true，
        -- 无文件无监听也能按天领保种奖励。connectable 为 NULL（未测）放行，
        -- 与做种收益结算同口径。
        -- P-559（2026-10-22 资深用户深测）：幂等键必须带「认领人 + 轮次」。
        -- 旧键 `preserve:<种号>:<档>` 把同一枚种的历次认领挤进同一条阶梯，而
        -- 「已发次数」按**当前认领人**算：种子「凑够人数出区 → 又死回区」
        -- （class_adj 的 ON CONFLICT 把 claimed_by 清零）或认领人被删号
        -- （user_del 置 NULL）之后换人接手，新一任数到自己 0 条 ⇒ 档号回到 1，
        -- 可撞键判据按**整枚种**算，第一任的 `preserve:<种>:1` 早在册 ⇒
        -- fresh 每一 tick 都把它洗掉：接手的人一天都领不到，且全程无报错。
        -- 现网事务内 A/B（夹具：上一任已领 3 档、现任认领满 3 天且在种）：
        -- 旧 SQL 应发 0 条、新 SQL 发出 `preserve:44020:1:<轮次>:1`。
        WITH cyc AS (
            SELECT sp.torrent_id, sp.claimed_by AS user_id,
                   'preserve:' || sp.torrent_id || ':' || sp.claimed_by || ':' ||
                     floor(extract(epoch FROM sp.claimed_at))::bigint || ':' AS prefix,
                   sp.claimed_at
            FROM seed_preserve sp
            JOIN snatches s
              ON s.torrent_id = sp.torrent_id AND s.user_id = sp.claimed_by
                 AND s.seeding
            WHERE sp.claimed_by IS NOT NULL AND sp.exited_at IS NULL
              -- 轮次取自 claimed_at：缺认领时间一律不结算。宁可这一档不发，也
              -- 不能让拼成 NULL 的 idempotency_key 绕过 trg_spark_idem 每小时重发
              AND sp.claimed_at IS NOT NULL
              -- 口径（0310）：仅 0(DEAD) 阻断；-2(SUSPECT)/-1(未测) 放行。
              AND s.last_port > 0 AND NOT COALESCE(s.connectable = 0, false)
        ),
        due AS (
            SELECT c.torrent_id, c.user_id, c.prefix,
                   (SELECT count(*) FROM spark_ledger l
                    WHERE l.kind = 'preserve_reward' AND l.user_id = c.user_id
                      AND l.idempotency_key LIKE c.prefix || '%') + 1 AS day_index,
                   COALESCE((SELECT max(l.created_at) FROM spark_ledger l
                    WHERE l.kind = 'preserve_reward' AND l.user_id = c.user_id
                      AND l.idempotency_key LIKE c.prefix || '%'),
                    c.claimed_at) AS last_paid_at
            FROM cyc c
        ),
        fresh AS (
            SELECT d.* FROM due d
            WHERE d.day_index >= 1
              AND now() - d.last_paid_at >= interval '1 day'
              AND NOT EXISTS (
                  SELECT 1 FROM spark_ledger l
                  WHERE l.idempotency_key = d.prefix || d.day_index
              )
        )
        INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id,
                                  idempotency_key)
        SELECT nextval('spark_ledger_id_seq'), user_id, $1, 'preserve_reward',
               'torrent', torrent_id, prefix || day_index
        FROM fresh
        "#,
    )
    .bind(bonus)
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}
