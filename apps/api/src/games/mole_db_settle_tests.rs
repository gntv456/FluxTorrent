//! 打地鼠 DB 级测试·补结算（直调生产函数 `settle_stale_rounds_tx`，
//! 不抄 handler 的 SQL —— 抄一遍的复刻件测不到 handler 的新逻辑）。
//!
//! 背景：切标签页 / 卸载会让前端提前调 finish，此前那一局当场按已命中的
//! 分数结算（后面没冒头的地鼠全部作废）。现在 `mole_finish` 拒早结，旧局
//! 留在未结算态，由 `mole_start` 开新局前补结算。这里锁的就是补结算的
//! 三条路径：已走完的要付、坏排程的也要付、进行中的不许碰。
//!
//! 跑法与门控同 mole_db_tests.rs 头注。

use serde_json::json;

use super::mole_db_harness as h;
use crate::games_http::settle_stale_rounds_tx;

/// 造一局并把 `created_at` 拨回 `secs_ago` 秒前；返回 round_id。
async fn aged_round(
    db: &sqlx::PgPool,
    uid: i64,
    idem: &str,
    schedule: serde_json::Value,
    secs_ago: i64,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO arcade_mole_rounds \
            (user_id, bet, schedule, idem, created_at) \
         VALUES ($1, 100, $2, $3, now() - make_interval(secs => $4)) \
         RETURNING id",
    )
    .bind(uid)
    .bind(schedule)
    .bind(idem)
    .bind(secs_ago)
    .fetch_one(db)
    .await
    .expect("aged round")
}

async fn is_resolved(db: &sqlx::PgPool, rid: i64) -> Option<(bool, i64, i64)> {
    sqlx::query_as(
        "SELECT resolved, COALESCE(score, 0), COALESCE(payout, 0) \
           FROM arcade_mole_rounds WHERE id = $1",
    )
    .bind(rid)
    .fetch_optional(db)
    .await
    .expect("round row")
}

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn finished_stale_round_is_settled_and_paid() {
    let db = h::pool().await;
    let (uid, bal0) = h::mk_user(&db, 10_000).await;
    // 一局两分钟前开的：敲中一只 +50 灰鼠、一只炸弹 -25，然后玩家切走了
    let idem = format!("mole:{uid}:stale-ok");
    let rid = aged_round(
        &db,
        uid,
        &idem,
        h::mole_json(&[h::grey(0), h::bomb(1), h::grey(2)]),
        120,
    )
    .await;
    h::whack(&db, rid, 0, 50).await;
    h::whack(&db, rid, 1, -25).await;

    let mut tx = db.begin().await.expect("tx");
    let done = settle_stale_rounds_tx(&mut tx, uid).await.expect("补结算");
    tx.commit().await.expect("commit");

    assert_eq!(done, 1, "应恰好补结算一局");
    let row = is_resolved(&db, rid).await.expect("row");
    assert_eq!(row, (true, 25, 25), "分数 50−25=25，实付 25：{row:?}");
    // 钱真的到账了：余额 +25（票价不退，那是开局时扣的）
    assert_eq!(h::balance(&db, uid).await, bal0 + 25, "补结算须入账");
    // 图鉴：只记中奖地鼠，炸弹不记
    let tiers: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM arcade_pool_rounds \
          WHERE user_id = $1 AND game = 'mole' AND spark = 50",
    )
    .bind(uid)
    .fetch_one(&db)
    .await
    .expect("tiers");
    assert_eq!(tiers, 1, "图鉴应落一行灰鼠");
    h::cleanup(&db).await;
}

/// 补结算幂等：跑第二遍不重复入账（钱已付过，局已 resolved）。
#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn stale_settlement_never_double_pays() {
    let db = h::pool().await;
    let (uid, bal0) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:stale-twice");
    let rid =
        aged_round(&db, uid, &idem, h::mole_json(&[h::grey(0)]), 120).await;
    h::whack(&db, rid, 0, 50).await;

    for _ in 0..2 {
        let mut tx = db.begin().await.expect("tx");
        let _ = settle_stale_rounds_tx(&mut tx, uid).await.expect("补结算");
        tx.commit().await.expect("commit");
    }
    assert_eq!(h::balance(&db, uid).await, bal0 + 50, "第二遍不得再付一次");
    h::cleanup(&db).await;
}

/// 排程损坏（坏 JSON / 空）的旧局：`schedule_done` 自己永不认，必须由整局
/// 时长闸放行 —— 否则玩家已命中的分数会被「解析失败」卡成坏账。
#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn broken_schedule_stale_round_is_still_settled() {
    let db = h::pool().await;
    let (uid, bal0) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:stale-broken");
    let rid = aged_round(&db, uid, &idem, json!([]), 120).await;
    h::whack(&db, rid, 0, 80).await;

    let mut tx = db.begin().await.expect("tx");
    let done = settle_stale_rounds_tx(&mut tx, uid).await.expect("补结算");
    tx.commit().await.expect("commit");

    assert_eq!(done, 1, "空排程的旧局也要由时长闸结掉");
    assert_eq!(h::balance(&db, uid).await, bal0 + 80, "命中的分数照付");
    assert_eq!(
        is_resolved(&db, rid).await,
        Some((true, 80, 80)),
        "排程坏了但命中表还在"
    );
    h::cleanup(&db).await;
}

/// 开局不到 36 秒的进行中半局：补结算一个字都不能碰。
#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn in_progress_round_is_untouched() {
    let db = h::pool().await;
    let (uid, bal0) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:stale-live");
    let rid = aged_round(&db, uid, &idem, h::mole_json(&[h::grey(0)]), 5).await;
    h::whack(&db, rid, 0, 50).await;

    let mut tx = db.begin().await.expect("tx");
    let done = settle_stale_rounds_tx(&mut tx, uid).await.expect("补结算");
    tx.commit().await.expect("commit");

    assert_eq!(done, 0, "进行中的半局不许被补结算");
    assert_eq!(is_resolved(&db, rid).await, Some((false, 0, 0)));
    assert_eq!(h::balance(&db, uid).await, bal0, "半局不动账");
    h::cleanup(&db).await;
}

/// 35 秒前开的局：排程按内容早已走完（最后一只 ≈27.7s 收尾），但还没过
/// 整局时长闸 —— 靠内容判据放行，玩家不必等满 30s 之外的余量。
#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn round_just_past_schedule_is_settled_by_content() {
    let db = h::pool().await;
    let (uid, bal0) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:stale-content");
    // 粗筛窗口 = ROUND_MS + HIT_LATE_MS + 5s = 36s，取 37s 刚过线
    let rid = aged_round(
        &db,
        uid,
        &idem,
        // 真实节奏：首只 600ms 起，每 1900ms 一只，停留 1500ms
        h::mole_json(&[
            json!({"seq":0,"at":600,"up":1500,"hole":0,"idx":1,"rarity":1,
                   "label":"灰鼠","value":50}),
            json!({"seq":13,"at":600+13*1900,"up":1500,"hole":4,"idx":3,
                   "rarity":3,"label":"粉猪鼠","value":200}),
        ]),
        37,
    )
    .await;
    h::whack(&db, rid, 13, 200).await;

    let mut tx = db.begin().await.expect("tx");
    let done = settle_stale_rounds_tx(&mut tx, uid).await.expect("补结算");
    tx.commit().await.expect("commit");

    assert_eq!(done, 1, "排程已走完的局应被结掉");
    assert_eq!(h::balance(&db, uid).await, bal0 + 200);
    h::cleanup(&db).await;
}
