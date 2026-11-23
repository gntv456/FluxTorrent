//! 打地鼠 DB 级测试的共享夹具（`mod mole_db_tests` 私有件）。
//!
//! harness 只做三件事：
//!   · 连库（DATABASE_URL 缺省 = apps/api/.env 里的本地 flux-postgres）
//!   · 造/清测试用户（`__mole_db_t` 前缀 + users 级联收尾）
//!   · 与 handler 同款的 start/finish SQL 复刻件（直接调生产入口
//!     spend_spark_tx / earn_spark_tx，保证测的就是上线那笔账）
//!
//! 跑法见 mole_db_tests.rs 头注。

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

/// 测试用户名前缀：收尾按它扫删；撞上真实用户名的概率约等于零。
pub(super) const TAG: &str = "__mole_db_t";

pub(super) async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://flux:_YLzbU1D8ktsL0Kz4NSqycqRkgrbhW5r\
         @127.0.0.1:5432/fluxtorrent"
            .into()
    });
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("连不上测试库：先起 flux-postgres（docker compose up -d）")
}

/// 建一个带余额的测试用户，返回 (user_id, 初始余额)。
pub(super) async fn mk_user(db: &PgPool, balance: i64) -> (i64, i64) {
    let mut tx = db.begin().await.expect("tx");
    // 每轮测试用不同的用户名（进程 id + 毫秒），重复跑不撞 UNIQUE
    let name = format!("{TAG}{}{}", std::process::id(), millis());
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, email, pass_hash, passkey, \
                               spark_balance) \
         VALUES ($1, $1 || '@dbt.invalid', 'x', \
                 gen_random_uuid()::text::char(32), $2) RETURNING id",
    )
    .bind(&name)
    .bind(balance)
    .fetch_one(&mut *tx)
    .await
    .expect("建测试用户");
    tx.commit().await.expect("commit");
    (id, balance)
}

fn millis() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

/// 清理：users 级联带走 arcade_mole_rounds/hits 与 arcade_pool_rounds；
/// spark_ledger 无外键，按 idem 前缀显式清。
pub(super) async fn cleanup(db: &PgPool) {
    // start 键（mole:uid:tag）与结算 win 键（game-mole-win:mole:uid:tag）
    sqlx::query("DELETE FROM spark_ledger WHERE idempotency_key LIKE $1")
        .bind(format!("%{TAG}%"))
        .execute(db)
        .await
        .expect("清 spark_ledger");
    sqlx::query("DELETE FROM users WHERE username LIKE $1")
        .bind(format!("{TAG}%"))
        .execute(db)
        .await
        .expect("清测试用户");
}

/// 与 mole.rs start 同款：扣票（幂等）→ 重放回读 or 落局。
pub(super) async fn start_round(
    db: &PgPool,
    uid: i64,
    ticket: i64,
    idem: &str,
    schedule: serde_json::Value,
) -> i64 {
    let mut tx = db.begin().await.expect("tx");
    let outcome = crate::economy_http::spend_spark_tx(
        &mut tx, uid, ticket, "game", idem, "mole", 0,
    )
    .await
    .expect("spend");
    let round_id: i64 = match outcome {
        crate::economy_http::SpendOutcome::Spent => sqlx::query_scalar(
            "INSERT INTO arcade_mole_rounds \
                    (user_id, bet, schedule, idem) \
                  VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(uid)
        .bind(ticket)
        .bind(&schedule)
        .bind(idem)
        .fetch_one(&mut *tx)
        .await
        .expect("insert round"),
        crate::economy_http::SpendOutcome::Replayed => sqlx::query_scalar(
            "SELECT id FROM arcade_mole_rounds \
                  WHERE user_id = $1 AND idem = $2 AND NOT resolved",
        )
        .bind(uid)
        .bind(idem)
        .fetch_one(&mut *tx)
        .await
        .expect("replay must find the round"),
    };
    tx.commit().await.expect("commit");
    round_id
}

/// 与 mole_settle.rs finish 同款：汇总命中 → earn + resolved + 图鉴
/// 同行事务；行数闸并发恰好一赢。
pub(super) async fn finish_round(
    db: &PgPool,
    uid: i64,
    round_id: i64,
) -> Result<(i64, i64), String> {
    let (_, resolved): (i64, bool) = sqlx::query_as(
        "SELECT bet, resolved FROM arcade_mole_rounds \
          WHERE id = $1 AND user_id = $2",
    )
    .bind(round_id)
    .bind(uid)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())?;
    if resolved {
        return Err("already-resolved".into());
    }
    let values_with_seq: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT seq, value FROM arcade_mole_hits WHERE round_id = $1",
    )
    .bind(round_id)
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    let values: Vec<i64> = values_with_seq.iter().map(|(_, v)| *v).collect();
    let (score, payout) = crate::games::settle_score(&values);
    let idem: String = sqlx::query_scalar(
        "SELECT COALESCE(idem, '') FROM arcade_mole_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(db)
    .await
    .map_err(|e| e.to_string())?;
    let win_idem = format!("game-mole-win:{idem}");
    let mut tx = db.begin().await.expect("tx");
    if payout > 0 {
        use crate::economy_http::earn_spark_tx;
        let _ = earn_spark_tx(&mut tx, uid, payout, "game", &win_idem)
            .await
            .map_err(|e| e.to_string())?;
    }
    let marked = sqlx::query(
        "UPDATE arcade_mole_rounds SET resolved = true, score = $2, \
                payout = $3, resolved_at = now() \
          WHERE id = $1 AND NOT resolved",
    )
    .bind(round_id)
    .bind(score)
    .bind(payout)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;
    if marked.rows_affected() == 0 {
        return Err("lost-race".into());
    }
    record_tiers(&mut tx, uid, round_id, &values_with_seq).await;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok((score, payout))
}

/// record_tiers 同款：逐只中奖地鼠落图鉴（炸弹 value<=0 跳过）。
async fn record_tiers(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    round_id: i64,
    hits: &[(i32, i64)],
) {
    let schedule: serde_json::Value = sqlx::query_scalar(
        "SELECT schedule FROM arcade_mole_rounds WHERE id = $1",
    )
    .bind(round_id)
    .fetch_one(&mut **tx)
    .await
    .expect("schedule");
    let spawns: Vec<crate::games::Spawn> =
        serde_json::from_value(schedule).unwrap_or_default();
    for (seq, value) in hits {
        if *value <= 0 {
            continue;
        }
        let Some(s) = spawns.iter().find(|s| s.seq == *seq) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO arcade_pool_rounds \
                (user_id, game, entry_index, rarity, prize, spark, value) \
             VALUES ($1, 'mole', $2, $3, $4, $5, $5)",
        )
        .bind(uid)
        .bind(i64::from(s.idx))
        .bind(s.rarity)
        .bind(&s.label)
        .bind(value)
        .execute(&mut **tx)
        .await
        .expect("tier");
    }
}

/// 排程夹具：与 build_schedule 产物同形（[{seq,at,up,hole,idx,...}]）。
pub(super) fn mole_json(spawns: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!(spawns)
}

/// 一只 +50 灰鼠（seq 自定）。
pub(super) fn grey(seq: i32) -> serde_json::Value {
    serde_json::json!({
        "seq": seq, "at": seq as i64 * 1900, "up": 1500,
        "hole": (seq % 3) as i64, "idx": 1, "rarity": 1,
        "label": "灰鼠", "value": 50,
    })
}

/// 炸弹（-25）。
pub(super) fn bomb(seq: i32) -> serde_json::Value {
    serde_json::json!({
        "seq": seq, "at": seq as i64 * 1900, "up": 1500,
        "hole": (seq % 3) as i64, "idx": -1, "rarity": 0,
        "label": "炸弹", "value": -25,
    })
}

/// 敲一发（窗口校验前的落表口径：主键防重）。
pub(super) async fn whack(db: &PgPool, rid: i64, seq: i32, value: i64) {
    sqlx::query(
        "INSERT INTO arcade_mole_hits (round_id, seq, value) \
         VALUES ($1, $2, $3) ON CONFLICT (round_id, seq) DO NOTHING",
    )
    .bind(rid)
    .bind(seq)
    .bind(value)
    .execute(db)
    .await
    .expect("hit");
}

/// 读用户余额。
pub(super) async fn balance(db: &PgPool, uid: i64) -> i64 {
    sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
        .bind(uid)
        .fetch_one(db)
        .await
        .expect("balance")
}
