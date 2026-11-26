//! 流量积分（0525，对照 StageAbove 口径）：上传/下载量按 GB 换积分的
//! 独立收益线。与做种结算（seeding.rs）完全解耦——做种是乘数链 +
//! arctan 封顶，流量是简单线性累加，不进封顶曲线。
//!
//! 公式（参数走 site_settings，与 API 预估 my_spark.rs 同源）：
//!   points = floor( Δ上传/1GB × traffic_up_rate + Δ下载/1GB × traffic_down_rate )
//!   Δ = 当前值 - traffic_settle_state 基线（GREATEST 钳 0：
//!   管理修正/数据回滚导致的倒退不产生负流水）。
//!   points=0 的行不落账也不推基线（自然滚存到下次，等价
//!   「不足最低结算额累计到下次」）；points≥1 才落账并推进基线。
//!
//! 幂等键 `traffic:{uid}:{yyyymmddhh}`，kind='traffic_reward'
//! （独立 kind：game_coupons 按 kind='seeding_reward' 判每日做种券，
//! 混入会污染口径，同 seeding_referral 的拆分理由）。
//! 余额快照由 seeding.rs 的「近 2 小时有流水」重算段覆盖（按 user_id
//! 聚合不筛 kind），无需重复实现。

use sqlx::PgPool;

/// 每用户流量积分小时结算。首见用户先建基线（=当前值，不补历史流量），
/// 之后每小时对 delta>0 且积分≥1 的用户落账。
pub(crate) async fn traffic_reward(db: &PgPool) -> anyhow::Result<u64> {
    let hour = chrono::Utc::now().format("%Y%m%d%H").to_string();
    // 基线补齐：无 state 行的用户以当前量建档（首月不补历史，
    // 与 StageAbove「开户起算」口径一致）。
    sqlx::query(
        "INSERT INTO traffic_settle_state (user_id, last_uploaded, \
         last_downloaded) \
         SELECT u.id, u.uploaded, u.downloaded FROM users u \
         WHERE u.status < 2 \
           AND NOT EXISTS ( \
               SELECT 1 FROM traffic_settle_state t \
               WHERE t.user_id = u.id) \
         ON CONFLICT (user_id) DO NOTHING",
    )
    .execute(db)
    .await?;
    // 落账 + 推基线：单条语句两步——先 INSERT（data-modifying CTE，
    // 幂等键挡重跑），主语句 UPDATE 基线且只对「本语句刚插入」的用户
    // 推进（FROM ins JOIN delta 同口径）。注意 PG 同语句内 data-modifying
    // CTE 与主语句读同一快照：UPDATE SET 里引用 delta 的字节量是本语句
    // 开始时的差值，与 INSERT 的计分完全一致，无竞态窗口。
    // 推进量 = 结算的 delta 原值（GB 零头随 floor 丢弃，不滚存——
    // 与「floor 后零头丢弃」的 StageAbove 口径一致）。
    let res = sqlx::query(
        r#"
        WITH rates AS (
            SELECT COALESCE(
                       (SELECT value::double precision FROM site_settings
                        WHERE name = 'seeding_traffic_up_rate'), 0.5) AS up,
                   COALESCE(
                       (SELECT value::double precision FROM site_settings
                        WHERE name = 'seeding_traffic_down_rate'), 0.1) AS down
        ),
        delta AS (
            SELECT u.id AS user_id,
                   GREATEST(u.uploaded - t.last_uploaded, 0)
                     ::double precision AS up_bytes,
                   GREATEST(u.downloaded - t.last_downloaded, 0)
                     ::double precision AS down_bytes,
                   rates.up, rates.down
            FROM users u
            JOIN traffic_settle_state t ON t.user_id = u.id
            CROSS JOIN rates
            WHERE u.status < 2
        ),
        scored AS (
            SELECT user_id, up_bytes, down_bytes,
                   floor(up_bytes / 1073741824.0 * up
                       + down_bytes / 1073741824.0 * down)::bigint AS points
            FROM delta
        ),
        -- 幂等：本小时已落过 traffic:{uid}:{hour} 的用户剔除（重跑安全）
        pay AS (
            SELECT s.* FROM scored s
            WHERE s.points >= 1
              AND NOT EXISTS (
                  SELECT 1 FROM spark_ledger l
                  WHERE l.idempotency_key = 'traffic:' || s.user_id || ':' || $1
              )
        ),
        ins AS (
            INSERT INTO spark_ledger
              (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), p.user_id, p.points,
                   'traffic_reward', 'traffic:' || p.user_id || ':' || $1
            FROM pay p
            RETURNING user_id
        )
        -- 基线推进：只推刚落账的用户（FROM ins），推进量 = 结算的 delta 原值
        UPDATE traffic_settle_state t
        SET last_uploaded = t.last_uploaded + d.up_bytes::bigint,
            last_downloaded = t.last_downloaded + d.down_bytes::bigint,
            updated_at = now()
        FROM ins
        JOIN delta d ON d.user_id = ins.user_id
        WHERE t.user_id = ins.user_id
        "#,
    )
    .bind(&hour)
    .execute(db)
    .await?;
    // 余额快照重算：只对本小时 traffic_reward 流水用户（与 seeding.rs
    // 的 2 小时窗口口径一致，这里窗口更窄开销更小）。
    sqlx::query(
        "UPDATE users SET spark_balance = COALESCE(( \
             SELECT base_spark FROM balance_baseline \
             WHERE user_id = users.id), 0) \
           + COALESCE(( \
             SELECT sum(amount) FROM spark_ledger \
             WHERE user_id = users.id AND kind <> 'orphan_offset'), 0) \
         WHERE id IN (SELECT DISTINCT user_id FROM spark_ledger \
                      WHERE kind = 'traffic_reward' \
                        AND created_at > now() - interval '2 hours') \
           AND COALESCE((SELECT base_spark FROM balance_baseline \
                         WHERE user_id = users.id), 0) \
             + COALESCE((SELECT sum(amount) FROM spark_ledger \
                         WHERE user_id = users.id \
                           AND kind <> 'orphan_offset'), 0) >= 0",
    )
    .execute(db)
    .await?;
    Ok(res.rows_affected())
}
