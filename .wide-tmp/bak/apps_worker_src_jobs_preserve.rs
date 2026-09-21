//! 保种结算。
//! 从 jobs.rs 按域拆出。

use sqlx::PgPool;

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
