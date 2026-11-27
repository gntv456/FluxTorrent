//! 斗地主 PvP 镜像同步 DB 级测试（从 pvp_db_tests 拆出：门禁）。
//! 跑法：`cargo test -p flux-api doudizhu_db -- --ignored --test-threads=1`

use super::doudizhu_db_harness::{cleanup, mk_user, pool};
use super::doudizhu_game::Phase;
use crate::games::doudizhu_repo::{
    apply_action_tx, load_active, load_by_id_public,
};

/// 镜像同步：A 叫 3 定候选地主 → B/C 行同步进抢地主且 landlord 一致；
/// A 走完弃牌/加倍到出牌阶段 → B/C 行同步进 Playing；A 出牌 → 事件同步。
///
/// 已知缺口（P2-1 新状态机阶段未适配镜像坐标系）：Double 阶段起始 turn 在
/// `discard()` 里写死行内 0，而镜像三行的行内 0 指向不同物理玩家；轮转事件
/// 的 seat 经轮换变换后与 peer 行内 turn 指针脱节，加倍三票无法在镜像行凑齐，
/// peer 停在 Double。bid/grab/discard/play 链路已同步，仅剩加倍与抢地主多票
/// 轮转需在状态机/重放层统一物理座位坐标后补齐。
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
            &mut tx, *uid, 100, "classic", "public",
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
    // 三行各自 state：A 行叫 3 → 新状态机进入抢地主环节（Grab）
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
    let mut row_a =
        apply_action_tx(&mut tx, uid_a, gid_a, "bid", Some(3), &[], 0)
            .await
            .unwrap();
    tx.commit().await.unwrap();
    assert!(
        matches!(
            row_a.state.phase,
            Phase::Grab | Phase::Discard | Phase::Double | Phase::Playing
        ),
        "叫 3 后应进入抢地主之后的环节"
    );
    // 同步验证：B/C 行同样进 Grab、landlord 与 A 行一致
    for (peer_idx, (gid, uid)) in rows.iter().enumerate().skip(1) {
        let mut tx = db.begin().await.unwrap();
        let peer = load_by_id_public(&mut tx, *uid, *gid).await.unwrap();
        tx.commit().await.unwrap();
        assert!(
            matches!(
                peer.state.phase,
                Phase::Grab | Phase::Discard | Phase::Double | Phase::Playing
            ),
            "镜像行应同步在抢地主之后环节"
        );
        // 行内座位编号是本行视角：换算回物理发牌序再比
        // （行 idx 按 id 序=发牌序；物理 = (行内座位 + 行 idx) % 3）
        let peer_phys =
            peer.state.landlord.map(|l| ((l as usize) + peer_idx) % 3);
        let a_phys = row_a.state.landlord.map(|l| l as usize);
        assert_eq!(peer_phys, a_phys, "地主应指向同一物理玩家");
    }

    // 推进 A 行走完抢地主/弃三张/加倍到 Playing（对手段由 AI 段代打）
    let mut guard = 0;
    while row_a.state.phase != Phase::Playing
        && row_a.state.phase != Phase::Done
    {
        guard += 1;
        assert!(guard < 100, "镜像对局推进超时");
        let Some(active) = load_active(&db, uid_a).await.unwrap() else {
            break;
        };
        if !active.my_turn {
            break;
        }
        let (action, score, cards) = match active.state.phase {
            Phase::Grab => ("grab", Some(0), vec![]),
            Phase::Discard => ("discard", None, vec![]),
            Phase::Double => ("double", Some(0), vec![]),
            Phase::Bidding => ("bid", Some(0), vec![]),
            _ => break,
        };
        let mut tx = db.begin().await.unwrap();
        row_a =
            apply_action_tx(&mut tx, uid_a, gid_a, action, score, &cards, 0)
                .await
                .unwrap();
        tx.commit().await.unwrap();
    }
    assert_eq!(row_a.state.phase, Phase::Playing, "应推进到出牌阶段");
    // 镜像 B/C：也进 Playing、地主一致
    for (peer_idx, (gid, uid)) in rows.iter().enumerate().skip(1) {
        let mut tx = db.begin().await.unwrap();
        let peer = load_by_id_public(&mut tx, *uid, *gid).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(peer.state.phase, Phase::Playing, "镜像行应同步进 playing");
        let peer_phys =
            peer.state.landlord.map(|l| ((l as usize) + peer_idx) % 3);
        let a_phys = row_a.state.landlord.map(|l| l as usize);
        assert_eq!(peer_phys, a_phys, "地主应指向同一物理玩家");
    }

    // A（地主）出第一手最小单张 → B/C 行 events 同步
    let lead = row_a.state.hands[0].last().copied().expect("hand has card");
    let mut tx = db.begin().await.unwrap();
    let row_a2 =
        apply_action_tx(&mut tx, uid_a, gid_a, "play", None, &[lead], 0)
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
        let peer = load_by_id_public(&mut tx, *uid, *gid).await.unwrap();
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
