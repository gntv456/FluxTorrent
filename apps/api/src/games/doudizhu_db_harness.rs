//! 斗地主 DB 级测试夹具（`doudizhu_db_tests` 私有件，仿 mole_db_harness）。
//! 连本地库 + 造/清 `__dz_db_t` 前缀测试用户；直接调生产入口
//! `doudizhu_repo::*_tx`，保证测的就是上线那笔账。

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::atomic::{AtomicU64, Ordering};

/// 用户名序号：同毫秒建多个测试用户时避免重名撞唯一索引。
static USER_SEQ: AtomicU64 = AtomicU64::new(0);

/// 测试用户名前缀：收尾按它扫删。
pub(super) const TAG: &str = "__dz_db_t";

pub(crate) async fn pool() -> PgPool {
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
    let seq = USER_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = format!("{TAG}{}{}{}", std::process::id(), millis(), seq);
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

/// 清理：users 级联带走 arcade_doudizhu_games；spark_ledger 按 idem 前缀清。
/// cleanup 按共享 TAG 全前缀扫删，跨测试有数据耦合——只许 `--test-threads=1`
/// 串行跑，并行会互删对方用户（`pvp_room_full_flow` 全红即为误用信号）。
pub(super) async fn cleanup(db: &PgPool) {
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
