//! 打地鼠 DB 级测试·经济侧（图鉴落行 / 过期局清理 / 库内播种池满盘闸）。
//! 跑法与门控同 mole_db_tests.rs 头注。

use super::mole_db_harness as h;

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn record_tiers_skips_bombs_and_zero() {
    let db = h::pool().await;
    let (uid, _) = h::mk_user(&db, 10_000).await;
    let idem = format!("mole:{uid}:tiers");
    // 排程：seq0=灰鼠，seq1=炸弹，seq2=另一只灰鼠（未敲，不进图鉴）
    let schedule = h::mole_json(&[h::grey(0), h::bomb(1), h::grey(2)]);
    let rid = h::start_round(&db, uid, 100, &idem, schedule).await;
    // 命中：灰鼠 + 炸弹（图鉴不该记炸弹）
    h::whack(&db, rid, 0, 50).await;
    h::whack(&db, rid, 1, -25).await;
    // finish 走一遍完整路径（earn + resolved + 图鉴）
    let _ = h::finish_round(&db, uid, rid).await.expect("finish");

    let tiers: Vec<(String, i64)> = sqlx::query_as(
        "SELECT prize, spark FROM arcade_pool_rounds \
          WHERE user_id = $1 AND game = 'mole'",
    )
    .bind(uid)
    .fetch_all(&db)
    .await
    .expect("tiers");
    assert_eq!(tiers.len(), 1, "炸弹不进图鉴：{tiers:?}");
    assert_eq!(tiers[0], ("灰鼠".into(), 50));
    h::cleanup(&db).await;
}

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn stale_unresolved_round_is_cleaned_by_start() {
    let db = h::pool().await;
    let (uid, _) = h::mk_user(&db, 10_000).await;
    // 造一条一小时前的未结算局（handler 的清理口径：created_at < now()-1h）
    let old_id: i64 = sqlx::query_scalar(
        "INSERT INTO arcade_mole_rounds (user_id, bet, schedule, created_at) \
         VALUES ($1, 100, '[]'::jsonb, now() - interval '61 minutes') \
         RETURNING id",
    )
    .bind(uid)
    .fetch_one(&db)
    .await
    .expect("old round");
    // 新鲜局必须活着
    let fresh_id: i64 = sqlx::query_scalar(
        "INSERT INTO arcade_mole_rounds (user_id, bet, schedule) \
         VALUES ($1, 100, '[]'::jsonb) RETURNING id",
    )
    .bind(uid)
    .fetch_one(&db)
    .await
    .expect("fresh round");

    let deleted = sqlx::query(
        "DELETE FROM arcade_mole_rounds \
          WHERE user_id = $1 AND NOT resolved \
            AND created_at < now() - interval '1 hour'",
    )
    .bind(uid)
    .execute(&db)
    .await
    .expect("cleanup");
    assert_eq!(deleted.rows_affected(), 1, "只清过期那一条");

    let left: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM arcade_mole_rounds WHERE id IN ($1, $2)",
    )
    .bind(old_id)
    .bind(fresh_id)
    .fetch_one(&db)
    .await
    .expect("count");
    assert_eq!(left, 1, "新鲜局必须还在");
    h::cleanup(&db).await;
}

#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn seeded_pool_in_db_passes_full_board_gate() {
    // 库里实际播种的池必须过满盘闸：防「库被后台改宽而代码单测还绿」
    let db = h::pool().await;
    let ticket: i64 = sqlx::query_scalar(
        "SELECT ticket FROM arcade_pools WHERE key = 'mole_default'",
    )
    .fetch_optional(&db)
    .await
    .expect("pool row")
    .expect("mole_default 池不存在（0511 未应用？）");
    let ev: f64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(weight * mult_permille), 0)::float8 \
                / 1000 / 1000 \
           FROM arcade_pool_entries \
          WHERE pool_key = 'mole_default' AND enabled",
    )
    .fetch_one(&db)
    .await
    .expect("ev");
    assert!(ev < 1.0, "单次 EV {ev} 必须 < 1");
    let full = ev * 12.0;
    assert!(full < 0.95, "满盘 {full} 必须 < 0.95 × 票价口径");
    assert!(ticket > 0, "票价 >0 已由 CHECK 约束保证，这里再显式看一眼");
}
