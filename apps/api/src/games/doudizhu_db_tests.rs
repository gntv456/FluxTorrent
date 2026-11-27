//! 斗地主 DB 级测试：开局扣款/幂等重放/动作推进/终局结算/派彩键。
//!
//! CI 无库铁律：全部 `#[ignore]` 门控。本地实连跑法：
//! `cargo test -p flux-api doudizhu_db -- --ignored --test-threads=1`
//! （夹具 `doudizhu_db_harness`；连不上库 = 先起 flux-postgres 容器）。

use super::doudizhu_db_harness::{cleanup, mk_user, pool, TAG};
use super::doudizhu_game::Phase;
use crate::games::doudizhu_repo::{
    apply_action_tx, load_active, start_game_tx,
};

/// 开局：扣底分 + 建局 + AI 叫分段推进。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn start_debits_and_creates_game() {
    let db = pool().await;
    cleanup(&db).await;
    let (uid, balance) = mk_user(&db, 10_000).await;
    let idem = format!("game-dz:{}:t1", uid);
    let mut tx = db.begin().await.unwrap();
    let row = start_game_tx(&mut tx, uid, 100, &idem, "classic", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    // 扣款落账
    let bal: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id=$1")
            .bind(uid)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(bal, balance - 100);
    let ledger: (String, i64) = sqlx::query_as(
        "SELECT ref_type, amount FROM spark_ledger WHERE idempotency_key=$1",
    )
    .bind(&idem)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(ledger, ("doudizhu".into(), -100));
    // 局已建、叫分段推进（AI 叫分：玩家 0 分也会被 AI 走到玩家轮或终局）
    assert_eq!(row.stake, 100);
    assert_ne!(row.state.phase, Phase::Done);
    // 幂等重放：同键再开，返回同一局不重扣
    let mut tx = db.begin().await.unwrap();
    let row2 = start_game_tx(&mut tx, uid, 100, &idem, "classic", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(row.id, row2.id);
    let bal: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id=$1")
            .bind(uid)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(bal, balance - 100, "重放不得再扣");
    cleanup(&db).await;
}

/// 完整对局：一路玩家 0 叫 + AI 推进 + 玩家托管到底 + 结算行与派彩键落库。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn full_game_settles_with_ledger() {
    let db = pool().await;
    cleanup(&db).await;
    let (uid, balance) = mk_user(&db, 100_000).await;
    let idem = format!("game-dz:{}:t2", uid);
    let mut tx = db.begin().await.unwrap();
    let mut row = start_game_tx(&mut tx, uid, 100, &idem, "classic", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    // 玩家全程「能过就过、轮到就叫 0」打到终局（超时托管的懒路径）
    let mut guard = 0;
    loop {
        guard += 1;
        assert!(guard < 500, "对局死循环");
        let Some(active) = load_active(&db, uid).await.unwrap() else {
            break;
        };
        if !active.my_turn {
            break; // 终局或异常
        }
        let mut tx = db.begin().await.unwrap();
        // 按新状态机全阶段分发：叫分 / 抢地主(不抢) / 弃三张(空弃) /
        // 加倍(不倍) / 出牌(过牌或最小)。此前只分 Bidding/play，
        // 轮到玩家处于 Grab/Discard/Double 时误发 play 被状态机拒、提前退出。
        let action = match active.state.phase {
            Phase::Bidding => ("bid", Some(0), vec![]),
            Phase::Grab => ("grab", Some(0), vec![]),
            Phase::Discard => ("discard", None, vec![]),
            Phase::Double => ("double", Some(0), vec![]),
            _ => ("play", None, vec![]),
        };
        let res = apply_action_tx(
            &mut tx, uid, active.id, action.0, action.1, &action.2, 0,
        )
        .await;
        match res {
            Ok(r) => {
                tx.commit().await.unwrap();
                row = r;
            }
            Err(_) => {
                tx.rollback().await.unwrap();
                break;
            }
        }
    }
    // 结算行：status settled、快照完整
    let (status, role, payout, net): (String, Option<String>, i64, i64) =
        sqlx::query_as(
            "SELECT status, role, payout, net FROM arcade_doudizhu_games \
         WHERE user_id=$1 ORDER BY id DESC LIMIT 1",
        )
        .bind(uid)
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(status, "settled", "对局应打到终局");
    assert!(role.is_some() || payout == 100, "流局=退款");
    // 派彩/退款键与余额对账
    let bal: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id=$1")
            .bind(uid)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(bal, balance - 100 + payout, "余额 = 初始 - 底分 + 派彩");
    let _ = net;
    cleanup(&db).await;
}

/// 癞子桌开局：variant=wild 必须初始化 wildcards（此前参数挂着却
/// 无条件走经典初始化，玩家选癞子实际拿到经典局）。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn start_wild_initializes_wildcards() {
    let db = pool().await;
    cleanup(&db).await;
    let (uid, _) = mk_user(&db, 10_000).await;
    let idem = format!("game-dz:{}:wild-init", uid);
    let mut tx = db.begin().await.unwrap();
    let row = start_game_tx(&mut tx, uid, 100, &idem, "wild", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    // state 快照里三座 wildcards 至少有一座非空（4 张癞子分布三家）
    let wildcards: serde_json::Value = sqlx::query_scalar(
        "SELECT state->'wildcards' FROM arcade_doudizhu_games WHERE id=$1",
    )
    .bind(row.id)
    .fetch_one(&db)
    .await
    .unwrap();
    let any_wild = [0usize, 1, 2]
        .iter()
        .any(|&i| wildcards[i].as_array().map_or(false, |a| !a.is_empty()));
    assert!(any_wild, "癞子桌必须至少有一座 wildcards 非空");
    // DB variant 列也应落 wild
    let variant: String = sqlx::query_scalar(
        "SELECT variant FROM arcade_doudizhu_games WHERE id=$1",
    )
    .bind(row.id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(variant, "wild");
    cleanup(&db).await;
}

/// 完整癞子局：玩家全程托管打到结算，验证 wildcards 登记、
/// 替身压牌、结算整链不卡死。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn full_wild_game_settles() {
    let db = pool().await;
    cleanup(&db).await;
    let (uid, balance) = mk_user(&db, 100_000).await;
    let idem = format!("game-dz:{}:wild-full", uid);
    let mut tx = db.begin().await.unwrap();
    start_game_tx(&mut tx, uid, 100, &idem, "wild", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut guard = 0;
    loop {
        guard += 1;
        assert!(guard < 500, "癞子局死循环");
        let Some(active) = load_active(&db, uid).await.unwrap() else {
            break;
        };
        if !active.my_turn {
            break;
        }
        let mut tx = db.begin().await.unwrap();
        let (action, score, cards) = match active.state.phase {
            Phase::Bidding => ("bid", Some(0), vec![]),
            Phase::Grab => ("grab", Some(0), vec![]),
            Phase::Discard => ("discard", None, vec![]),
            Phase::Double => ("double", Some(0), vec![]),
            _ => ("play", None, vec![]),
        };
        let res =
            apply_action_tx(&mut tx, uid, active.id, action, score, &cards, 0)
                .await;
        match res {
            Ok(_) => tx.commit().await.unwrap(),
            Err(_) => {
                tx.rollback().await.unwrap();
                break;
            }
        }
    }
    let (status, payout): (String, i64) = sqlx::query_as(
        "SELECT status, payout FROM arcade_doudizhu_games \
         WHERE user_id=$1 ORDER BY id DESC LIMIT 1",
    )
    .bind(uid)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(status, "settled", "癞子局应走到结算");
    let bal: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id=$1")
            .bind(uid)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(bal, balance - 100 + payout, "癞子局余额对平");
    cleanup(&db).await;
}

/// 并发开局：唯一活跃局索引兜底，第二笔被拒。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn concurrent_start_rejected_by_unique_index() {
    let db = pool().await;
    cleanup(&db).await;
    let (uid, _) = mk_user(&db, 10_000).await;
    let idem_a = format!("game-dz:{}:ca", uid);
    let mut tx = db.begin().await.unwrap();
    start_game_tx(&mut tx, uid, 100, &idem_a, "classic", false)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let active = load_active(&db, uid).await.unwrap();
    assert!(active.is_some(), "首局应存在");
    // 直接插第二行活跃局：唯一部分索引应拒绝
    let dup = sqlx::query(
        "INSERT INTO arcade_doudizhu_games \
         (user_id, mode, status, stake, idempotency_key, state) \
         VALUES ($1, 'ai', 'bidding', 100, $2, '{}')",
    )
    .bind(uid)
    .bind(format!("{TAG}-dup"))
    .execute(&db)
    .await;
    assert!(dup.is_err(), "唯一活跃局索引应拦下第二局");
    cleanup(&db).await;
}
