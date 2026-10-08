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
///   site_settings 可调）；幂等键 = preserve:{torrent_id}:{day_index}（自认领起算的
///   整天序号），重跑/补跑安全。停做种期间不结算（恢复后在下个整天边界继续），
/// 时长基线列保留供后台展示 delta。
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
        -- 下一档 = 已结算 preserve 行数 + 1，停种期间不产生新行；
        -- 仍要求结算时点在种 + 距上一档 ≥1 天（防反复上下线刷首档）。
        -- P0-2（2026-10-07 保种组审计）：「在种」补幽灵过滤——port>0 且回连
        -- 非「不可达」。实测 port=0 的纯 curl announce 即可令 seeding=true，
        -- 无文件无监听也能按天领保种奖励。connectable 为 NULL（未测）放行，
        -- 与做种收益结算同口径。
        WITH due AS (
            SELECT sp.claimed_by AS user_id, sp.torrent_id,
                   (SELECT count(*) FROM spark_ledger l
                    WHERE l.kind = 'preserve_reward'
                      AND l.idempotency_key LIKE 'preserve:' || sp.torrent_id || ':%'
                      AND l.user_id = sp.claimed_by) + 1 AS day_index,
                   COALESCE((SELECT max(l.created_at) FROM spark_ledger l
                    WHERE l.kind = 'preserve_reward'
                      AND l.idempotency_key LIKE 'preserve:' || sp.torrent_id || ':%'
                      AND l.user_id = sp.claimed_by), sp.claimed_at) AS last_paid_at
            FROM seed_preserve sp
            JOIN snatches s
              ON s.torrent_id = sp.torrent_id AND s.user_id = sp.claimed_by AND s.seeding
            WHERE sp.claimed_by IS NOT NULL AND sp.exited_at IS NULL
              -- 口径（0310）：仅 0(DEAD) 阻断；-2(SUSPECT)/-1(未测) 放行。
              AND s.last_port > 0 AND NOT COALESCE(s.connectable = 0, false)
              AND now() - COALESCE((SELECT max(l.created_at) FROM spark_ledger l
                    WHERE l.kind = 'preserve_reward'
                      AND l.idempotency_key LIKE 'preserve:' || sp.torrent_id || ':%'
                      AND l.user_id = sp.claimed_by), sp.claimed_at) >= interval '1 day'
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
