//! 固定抽签 DB 级回归（CI 无库：全部 #[ignore]）。
//! 本地跑法：cargo test -p flux-api fixed_lottery_db -- --ignored --test-threads=1
//! 夹具直连生产表与生产函数，验证「库上真的如此」，不复制 SQL。

use super::claim::{expire_overdue, submit_db};
use crate::repo::fixed_lottery as rp;
use crate::repo::fixed_lottery_read as rr;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

const TAG: &str = "__flx_db_t";

async fn pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://flux:_YLzbU1D8ktsL0Kz4NSqycqRkgrbhW5r\
         @127.0.0.1:5432/fluxtorrent"
            .into()
    });
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&url)
        .await
        .expect("连不上测试库：先起 flux-postgres")
}

fn stamp() -> u128 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

async fn mk_user(db: &PgPool, tag: &str) -> i64 {
    let name = format!("{TAG}{tag}{}", stamp());
    sqlx::query_scalar(
        "INSERT INTO users \
         (username, email, pass_hash, passkey, spark_balance) \
         VALUES ($1, $1 || '@dbt.invalid', 'x', \
                 gen_random_uuid()::text::char(32), 0) RETURNING id",
    )
    .bind(&name)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn mk_campaign(db: &PgPool, owner: i64) -> i64 {
    let code = format!("FX{:06}", stamp() % 1_000_000);
    sqlx::query_scalar(
        "INSERT INTO fixed_lottery_campaigns \
         (participation_code, title, status, trigger_type, \
          claim_default_days, created_by) \
         VALUES ($1, 'db test', 'open', 'scheduled', 14, $2) \
         RETURNING id",
    )
    .bind(&code)
    .bind(owner)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn mk_prize(db: &PgPool, cid: i64, kind: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO fixed_lottery_prizes \
         (campaign_id, sort_order, name, quantity, reward_kind, \
          reward_config) \
         VALUES ($1, 0, 'db prize', 3, $2, \
                 '{\"fields\":[{\"key\":\"name\",\"required\":true}]}') \
         RETURNING id",
    )
    .bind(cid)
    .bind(kind)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn mk_winner(
    db: &PgPool,
    cid: i64,
    pid: i64,
    uid: i64,
    status: &str,
    expired: bool,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO fixed_lottery_winners \
         (campaign_id, prize_id, user_id, username_snapshot, \
          prize_name_snapshot, slot_no, draw_position, status, \
          claim_deadline_at) \
         VALUES ($1, $2, $3, 'u', 'p', \
                 (SELECT COALESCE(MAX(slot_no), 0) + 1 \
                  FROM fixed_lottery_winners WHERE campaign_id = $1), \
                 (SELECT COALESCE(MAX(draw_position), 0) + 1 \
                  FROM fixed_lottery_winners WHERE campaign_id = $1), \
                 $4, CASE WHEN $5 THEN now() - interval '1 day' \
                          ELSE now() + interval '1 day' END) \
         RETURNING id",
    )
    .bind(cid)
    .bind(pid)
    .bind(uid)
    .bind(status)
    .bind(expired)
    .fetch_one(db)
    .await
    .unwrap()
}

async fn cleanup(db: &PgPool) {
    sqlx::query(
        "DELETE FROM fixed_lottery_campaigns \
         WHERE participation_code LIKE 'FX%'",
    )
    .execute(db)
    .await
    .unwrap();
    sqlx::query("DELETE FROM users WHERE username LIKE $1")
        .bind(format!("{TAG}%"))
        .execute(db)
        .await
        .unwrap();
}

/// 开奖 CAS 之后，报名口必须被活动行锁挡住。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 --ignored --test-threads=1"]
async fn late_join_after_cas_rejected() {
    let db = pool().await;
    cleanup(&db).await;
    let uid = mk_user(&db, "join").await;
    let cid = mk_campaign(&db, uid).await;

    sqlx::query(
        "UPDATE fixed_lottery_campaigns SET status = 'drawn' WHERE id = $1",
    )
    .bind(cid)
    .execute(&db)
    .await
    .unwrap();

    let mut tx = db.begin().await.unwrap();
    let res = rp::join_tx(&mut tx, cid, uid, "u", None).await;
    assert!(res.is_err(), "开奖后报名必须被拒");
    tx.rollback().await.unwrap();
    cleanup(&db).await;
}

/// CAS 持锁后读到的候选集必须包含此前已提交的报名。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 --ignored --test-threads=1"]
async fn candidate_snapshot_after_cas_is_complete() {
    let db = pool().await;
    cleanup(&db).await;
    let u1 = mk_user(&db, "c1").await;
    let u2 = mk_user(&db, "c2").await;
    let cid = mk_campaign(&db, u1).await;

    let mut tx = db.begin().await.unwrap();
    rp::join_tx(&mut tx, cid, u1, "u1", None).await.unwrap();
    rp::join_tx(&mut tx, cid, u2, "u2", None).await.unwrap();
    tx.commit().await.unwrap();

    let mut tx = db.begin().await.unwrap();
    let n = sqlx::query(
        "UPDATE fixed_lottery_campaigns \
         SET status = 'drawn' WHERE id = $1 AND status = 'open'",
    )
    .bind(cid)
    .execute(&mut *tx)
    .await
    .unwrap()
    .rows_affected();
    assert_eq!(n, 1);
    let mut uids = rr::candidate_uids_tx(&mut tx, cid).await.unwrap();
    uids.sort_unstable();
    let mut expect = vec![u1, u2];
    expect.sort_unstable();
    assert_eq!(uids, expect, "快照必须含全部开奖前已提交报名");
    tx.rollback().await.unwrap();
    cleanup(&db).await;
}

/// 审核通过后重交必须被库上 CAS 拒绝，且不能改写 submission。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 --ignored --test-threads=1"]
async fn approved_claim_cannot_be_overwritten() {
    let db = pool().await;
    cleanup(&db).await;
    let uid = mk_user(&db, "claim").await;
    let cid = mk_campaign(&db, uid).await;
    let pid = mk_prize(&db, cid, "custom").await;
    let wid = mk_winner(&db, cid, pid, uid, "claimed", false).await;

    submit_db(&db, uid, wid, json!({"name": "first"}))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE fixed_lottery_claims SET status = 'approved' \
         WHERE winner_id = $1",
    )
    .bind(wid)
    .execute(&db)
    .await
    .unwrap();

    assert!(submit_db(&db, uid, wid, json!({"name": "second"}))
        .await
        .is_err());
    let (status, submission): (String, serde_json::Value) = sqlx::query_as(
        "SELECT status, submission FROM fixed_lottery_claims \
         WHERE winner_id = $1",
    )
    .bind(wid)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(status, "approved");
    assert_eq!(submission["name"], "first");
    cleanup(&db).await;
}

/// 超期扫描：satisfied/被驳回要失效，已提交待审要保留。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 --ignored --test-threads=1"]
async fn claim_expiry_respects_submission_state() {
    let db = pool().await;
    cleanup(&db).await;
    let owner = mk_user(&db, "expo").await;
    let cid = mk_campaign(&db, owner).await;
    let pid = mk_prize(&db, cid, "custom").await;

    let u1 = mk_user(&db, "e1").await;
    let u2 = mk_user(&db, "e2").await;
    let u3 = mk_user(&db, "e3").await;
    let w1 = mk_winner(&db, cid, pid, u1, "satisfied", true).await;
    let w2 = mk_winner(&db, cid, pid, u2, "claimed", true).await;
    let w3 = mk_winner(&db, cid, pid, u3, "claimed", true).await;
    for (w, st) in [(w2, "rejected"), (w3, "pending_review")] {
        sqlx::query(
            "INSERT INTO fixed_lottery_claims (winner_id, status) \
             VALUES ($1, $2)",
        )
        .bind(w)
        .bind(st)
        .execute(&db)
        .await
        .unwrap();
    }

    let n = expire_overdue(&db).await.unwrap();
    assert_eq!(n, 2, "satisfied 与 rejected 各翻一条");
    let st1: String = sqlx::query_scalar(
        "SELECT status FROM fixed_lottery_winners WHERE id = $1",
    )
    .bind(w1)
    .fetch_one(&db)
    .await
    .unwrap();
    let st2: String = sqlx::query_scalar(
        "SELECT status FROM fixed_lottery_winners WHERE id = $1",
    )
    .bind(w2)
    .fetch_one(&db)
    .await
    .unwrap();
    let st3: String = sqlx::query_scalar(
        "SELECT status FROM fixed_lottery_winners WHERE id = $1",
    )
    .bind(w3)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        (st1.as_str(), st2.as_str(), st3.as_str()),
        ("failed", "failed", "claimed")
    );
    let cs2: String = sqlx::query_scalar(
        "SELECT status FROM fixed_lottery_claims WHERE winner_id = $1",
    )
    .bind(w2)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(cs2, "expired");
    cleanup(&db).await;
}
