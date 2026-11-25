//! 斗地主 PvP 镜像同步 DB 级测试（从 pvp_db_tests 拆出：门禁）。
//! 跑法：`cargo test -p flux-api doudizhu_db -- --ignored --test-threads=1`

use super::doudizhu_db_harness::{cleanup, mk_user, pool};

/// 镜像同步：A 叫 3 当地主 → B/C 行同步进 playing 且 landlord 一致；
/// A 出牌 → B/C 行 events 同步推进（资深玩家抓到的架构级缺口）。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn pvp_mirror_events_fan_out() {
    let db = pool().await;
    cleanup(&db).await;
    let seats = [
        mk_user(&db, 10_000).await,
        mk_user(&db, 10_000).await,
        mk_user(&db, 10_000).await,
    ];
    let uids: Vec<i64> = seats.iter().map(|(u, _)| *u).collect();
    for uid in &uids {
        let mut tx = db.begin().await.unwrap();
        super::doudizhu_pvp_repo::join_or_create_tx(
            &mut tx, *uid, 100, "classic",
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    // 房主（第一座）开局
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::deal_room_tx(&mut tx, {
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM arcade_doudizhu_rooms ORDER BY id DESC LIMIT 1",
        )
        .fetch_one(&db)
        .await
        .unwrap()
    })
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // 三行各自 state：A 行叫 3
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM arcade_doudizhu_games \
         WHERE room_id IS NOT NULL ORDER BY id",
    )
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3);
    let (gid_a, uid_a) = rows[0];
    let mut tx = db.begin().await.unwrap();
    let row_a = super::doudizhu_repo::apply_action_tx(
        &mut tx,
        uid_a,
        gid_a,
        "bid",
        Some(3),
        &[],
        0,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        row_a.state.phase,
        crate::games::doudizhu_game::Phase::Playing
    );
    // 同步验证：B/C 行 phase=playing、landlord 与 A 行一致
    for (peer_idx, (gid, uid)) in rows.iter().enumerate().skip(1) {
        let mut tx = db.begin().await.unwrap();
        let peer = super::doudizhu_repo::load_by_id_public(&mut tx, *uid, *gid)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            peer.state.phase,
            crate::games::doudizhu_game::Phase::Playing,
            "镜像行应同步进 playing"
        );
        // 行内座位编号是本行视角：换算回物理发牌序再比
        // （行 idx 按 id 序=发牌序；物理 = (行内座位 + 行 idx) % 3）
        let peer_phys =
            peer.state.landlord.map(|l| ((l as usize) + peer_idx) % 3);
        let a_phys = row_a.state.landlord.map(|l| l as usize);
        assert_eq!(peer_phys, a_phys, "地主应指向同一物理玩家");
    }
    // A（地主）出第一手最小单张 → B/C 行 events 同步
    let mut tx = db.begin().await.unwrap();
    let row_a2 = super::doudizhu_repo::apply_action_tx(
        &mut tx,
        uid_a,
        gid_a,
        "play",
        None,
        &[row_a.state.hands[0].last().copied().expect("hand has card")],
        0,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let played = row_a2
        .state
        .events
        .iter()
        .rev()
        .find(|e| e.action == "play")
        .unwrap();
    for (gid, uid) in rows.iter().skip(1) {
        let mut tx = db.begin().await.unwrap();
        let peer = super::doudizhu_repo::load_by_id_public(&mut tx, *uid, *gid)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert!(
            peer.state
                .events
                .iter()
                .any(|e| e.action == "play" && e.cards == played.cards),
            "镜像行应同步到该次出牌"
        );
    }
    cleanup(&db).await;
}
