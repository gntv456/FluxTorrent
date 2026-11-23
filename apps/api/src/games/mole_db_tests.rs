//! 打地鼠 DB 级测试（`#[ignore]` 门控：CI 无数据库，本地实连跑）。
//!
//! 锁的是 handler 里那条**资金 SQL 链**的真实行为，不是复刻一遍逻辑：
//!   · 开局扣票 + 幂等重放不双扣（spend_spark_tx Replayed → 回读原局）
//!   · 命中主键判重（同一只敲两次只落一行）
//!   · 结算只按服务端命中派彩，余额对账 after-before = payout-ticket
//!   · 结算幂等（第二次 finish 原样回读，不再入账）
//!   · 并发双 finish 恰好一赢（UPDATE ... AND NOT resolved 的行数闸）
//!
//! 跑法：`cargo test -p flux-api mole_db -- --ignored --test-threads=1`
//! 连接串取 `DATABASE_URL`（缺省 = apps/api/.env 里的本地 flux-postgres）。
//! 测试用户以 `__mole_db_t` 前缀建、走 `ON DELETE CASCADE` 收尾，
//! 碰不到线上账号；spark_ledger 无外键，按 idem 前缀显式清。
//! 图鉴/清理/播种池三测在 mole_db_economy_tests.rs。

use super::mole_db_harness as h;

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn start_spends_once_and_replay_returns_same_round() {
    let db = h::pool().await;
    let (uid, before) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:replay-test");

    let schedule = h::mole_json(&[]);
    let r1 = h::start_round(&db, uid, 100, &idem, schedule.clone()).await;
    let r2 = h::start_round(&db, uid, 100, &idem, schedule).await;
    assert_eq!(r1, r2, "同幂等键重放必须回读同一局");

    let after = h::balance(&db, uid).await;
    assert_eq!(after, before - 100, "重放不得双扣票");

    // 结算侧的 win 幂等键从局上的 idem 派生，重放局结算也要能对上
    let stored: Option<String> =
        sqlx::query_scalar("SELECT idem FROM arcade_mole_rounds WHERE id = $1")
            .bind(r1)
            .fetch_one(&db)
            .await
            .expect("idem");
    assert_eq!(stored.as_deref(), Some(idem.as_str()));
    h::cleanup(&db).await;
}

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn hit_pk_blocks_duplicate_and_settle_pays_recorded_only() {
    let db = h::pool().await;
    let (uid, before) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:hit-test");
    // 排程：seq0=+50 地鼠，seq1=炸弹 -25（客户端谎报的 seq2 无从落表）
    let schedule = h::mole_json(&[h::grey(0), h::bomb(1)]);
    let rid = h::start_round(&db, uid, 100, &idem, schedule).await;

    // 敲中 seq0 两次：主键闸只落一行
    h::whack(&db, rid, 0, 50).await;
    h::whack(&db, rid, 0, 50).await;
    // 炸弹也敲中
    h::whack(&db, rid, 1, -25).await;

    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM arcade_mole_hits WHERE round_id = $1",
    )
    .bind(rid)
    .fetch_one(&db)
    .await
    .expect("count");
    assert_eq!(rows, 2, "同一只重复敲击只落一行");

    let (score, payout) = h::finish_round(&db, uid, rid).await.expect("finish");
    assert_eq!(score, 25, "50 + (-25)");
    assert_eq!(payout, 25, "派彩不为负：score<0 时 payout=0，此处=25");

    let after = h::balance(&db, uid).await;
    assert_eq!(after, before - 100 + payout, "after-before = payout-ticket");

    h::cleanup(&db).await;
}

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn finish_is_idempotent_and_never_double_pays() {
    let db = h::pool().await;
    let (uid, before) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:idem-finish");
    let schedule = h::mole_json(&[h::grey(0)]);
    let rid = h::start_round(&db, uid, 100, &idem, schedule).await;
    h::whack(&db, rid, 0, 50).await;

    let (_, payout1) = h::finish_round(&db, uid, rid).await.expect("finish1");
    assert_eq!(payout1, 50);
    // 第二次 finish：resolved=true → 拒绝（handler 回读已结算视图，不再入账）
    let err = h::finish_round(&db, uid, rid).await.unwrap_err();
    assert!(err.contains("already-resolved"), "err={err}");

    let after = h::balance(&db, uid).await;
    assert_eq!(after, before - 100 + 50, "不得二次入账");
    h::cleanup(&db).await;
}

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn concurrent_finish_has_exactly_one_winner() {
    let db = h::pool().await;
    let (uid, before) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:race-finish");
    let schedule = h::mole_json(&[h::grey(0)]);
    let rid = h::start_round(&db, uid, 100, &idem, schedule).await;
    h::whack(&db, rid, 0, 50).await;

    // 并发双 finish：行数闸保证恰好一笔 commit
    let d2 = db.clone();
    let d3 = db.clone();
    let j1 = tokio::spawn(async move { h::finish_round(&d2, uid, rid).await });
    let j2 = tokio::spawn(async move { h::finish_round(&d3, uid, rid).await });
    let (a, b) = tokio::join!(j1, j2);
    let outcomes = [a.expect("task1"), b.expect("task2")];
    let winners = outcomes.iter().filter(|r| r.is_ok()).count();
    assert_eq!(winners, 1, "并发双 finish 必须恰好一笔赢：{outcomes:?}");
    let total_paid: i64 = outcomes
        .iter()
        .filter_map(|r| r.as_ref().ok().map(|(_, p)| *p))
        .sum();
    assert_eq!(total_paid, 50, "只发一次奖");

    let after = h::balance(&db, uid).await;
    assert_eq!(after, before - 100 + 50, "并发不得双发");
    h::cleanup(&db).await;
}
