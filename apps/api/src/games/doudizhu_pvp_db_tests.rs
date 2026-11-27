//! 斗地主 PvP DB 级测试（从 doudizhu_db_tests.rs 拆出：主文件贴
//! 300 行门禁）。房间全链 / 开局扣款回滚 / 挂机代打。
//!
//! 跑法：`cargo test -p flux-api doudizhu_db -- --ignored --test-threads=1`

use super::doudizhu_db_harness::{apply_migrations, cleanup, mk_user, pool};

/// PvP 全链：建房 → 两人加入 → 房主开局 → 三行镜像局 + 座位唯一约束。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn pvp_room_full_flow() {
    let db = pool().await;
    cleanup(&db).await;
    let (a, _) = mk_user(&db, 10_000).await;
    let (b, _) = mk_user(&db, 10_000).await;
    let (c, _) = mk_user(&db, 10_000).await;
    // a 建房（坐 0）
    let mut tx = db.begin().await.unwrap();
    let (rid, seat_a, joined) = super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, a, 100, "wild", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(joined);
    assert_eq!(seat_a, 0);
    // b 加入（坐 1）
    let mut tx = db.begin().await.unwrap();
    let (rid_b, seat_b, _) = super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, b, 100, "wild", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(rid_b, rid);
    assert_eq!(seat_b, 1);
    // c 加入（坐 2）
    let mut tx = db.begin().await.unwrap();
    let (rid_c, seat_c, _) = super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, c, 100, "wild", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(rid_c, rid);
    assert_eq!(seat_c, 2);
    // 满员后同 variant 匹配会建房不挤桌（新房间）
    let (d, _) = mk_user(&db, 10_000).await;
    let mut tx = db.begin().await.unwrap();
    let (rid_d, _, joined_d) = super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, d, 100, "wild", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert!(joined_d);
    assert_ne!(rid_d, rid, "满员房不能再进人");
    // 房主开局 → 三行镜像局
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::deal_room_tx(&mut tx, rid)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT user_id, mode, variant FROM arcade_doudizhu_games \
         WHERE room_id = $1 ORDER BY id",
    )
    .bind(rid)
    .fetch_all(&db)
    .await
    .unwrap();
    assert_eq!(rows.len(), 3, "三行镜像局");
    assert!(rows.iter().all(|(_, m, v)| m == "pvp" && v == "wild"));
    // 三行手牌同源：各自 hands[0] 是不同 17 张（集合并=51 张去重后 51）
    let mut all = Vec::new();
    for gid in sqlx::query_scalar::<_, i64>(
        "SELECT id FROM arcade_doudizhu_games WHERE room_id = $1",
    )
    .bind(rid)
    .fetch_all(&db)
    .await
    .unwrap()
    {
        let state: serde_json::Value = sqlx::query_scalar(
            "SELECT state FROM arcade_doudizhu_games WHERE id = $1",
        )
        .bind(gid)
        .fetch_one(&db)
        .await
        .unwrap();
        let h = state["hands"][0].as_array().unwrap();
        all.extend(h.iter().map(|x| x.as_i64().unwrap()));
    }
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 51, "三份手牌正好铺开 51 张无重复");
    // 房状态转 playing
    let status: String = sqlx::query_scalar(
        "SELECT status FROM arcade_doudizhu_rooms WHERE id = $1",
    )
    .bind(rid)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(status, "playing");
    // 房主离开 playing 房：leave 只处理 waiting → 不动对局
    let mut tx = db.begin().await.unwrap();
    let left = super::doudizhu_pvp_repo::leave_room_tx(&mut tx, a)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(left.is_none(), "playing 房不许 leave 逃局");
    cleanup(&db).await;
}

/// PvP 经济闭环：开局事务里三座各扣底分；任一座余额不足 → 整桌回滚
/// （房仍 waiting，无人被扣）。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn pvp_deal_debits_all_three_or_rolls_back() {
    let db = pool().await;
    cleanup(&db).await;
    let (a, bal_a) = mk_user(&db, 10_000).await;
    let (b, bal_b) = mk_user(&db, 10_000).await;
    // c 余额不足（底分 100 但只有 50）
    let (c, bal_c) = mk_user(&db, 50).await;
    let mut tx = db.begin().await.unwrap();
    let (rid, ..) = super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, a, 100, "classic", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, b, 100, "classic", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, c, 100, "classic", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // 房主开局：c 余额不足 → 整笔回滚
    let mut tx = db.begin().await.unwrap();
    let res = super::doudizhu_pvp_repo::deal_room_tx(&mut tx, rid).await;
    assert!(res.is_err(), "余额不足应拒绝开局");
    drop(tx); // 回滚收尾
              // 三人余额全部原封（a/b 的预扣随事务回滚）
    for (uid, bal) in [(a, bal_a), (b, bal_b), (c, bal_c)] {
        let got: i64 =
            sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
                .bind(uid)
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(got, bal, "回滚后余额必须还原");
    }
    // 房间还在 waiting（可补足后重开）
    let status: String = sqlx::query_scalar(
        "SELECT status FROM arcade_doudizhu_rooms WHERE id = $1",
    )
    .bind(rid)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(status, "waiting");
    // 补足 c 余额后重开成功：三座各扣 100
    sqlx::query(
        "UPDATE users SET spark_balance = spark_balance + 100 WHERE id = $1",
    )
    .bind(c)
    .execute(&db)
    .await
    .unwrap();
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::deal_room_tx(&mut tx, rid)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let got_a: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(a)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(got_a, bal_a - 100);
    let games: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM arcade_doudizhu_games WHERE room_id = $1",
    )
    .bind(rid)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(games, 3);
    cleanup(&db).await;
}

/// 挂机懒托管：玩家轮次停摆（updated_at 回拨）后，任意动作请求触发
/// AI 代打到底并结算；赢家派彩落账、输家底分回收。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn pvp_idle_kicks_ai_to_settlement() {
    let db = pool().await;
    cleanup(&db).await;
    let (a, bal_a) = mk_user(&db, 10_000).await;
    let idem = format!("game-dz:{}:idle", a);
    // AI 房走同一条 apply_action_tx 管线（idle_secs 参数一致）
    let mut tx = db.begin().await.unwrap();
    let row = super::doudizhu_repo::start_game_tx(
        &mut tx, a, 100, &idem, "classic", false,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    // 回拨 updated_at 61 秒（阈值 60）模拟挂机
    sqlx::query(
        "UPDATE arcade_doudizhu_games \
         SET updated_at = now() - interval '61 seconds' WHERE id = $1",
    )
    .bind(row.id)
    .execute(&db)
    .await
    .unwrap();
    // 任意动作请求进来：先托管（可能直接代打完）
    let mut tx = db.begin().await.unwrap();
    let res = super::doudizhu_repo::apply_action_tx(
        &mut tx,
        a,
        row.id,
        "bid",
        Some(0),
        &[],
        60,
    )
    .await;
    tx.commit().await.unwrap();
    // 托管可能直接代打到终局：后续 bid 被拒（"还没轮到你"）是正常路径，
    // 只要求请求不 5xx（Validation 可接受）。
    if let Err(crate::errors::DomainError::Internal(e)) = &res {
        panic!("托管内部错误: {e:?}");
    }
    // 局最终走到 settled（代打到底）；流水只有开局扣款与派彩/退款
    let (status, payout): (String, i64) = sqlx::query_as(
        "SELECT status, payout FROM arcade_doudizhu_games WHERE id = $1",
    )
    .bind(row.id)
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(status, "settled", "挂机代打必须走到结算");
    let bal: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(a)
            .fetch_one(&db)
            .await
            .unwrap();
    assert_eq!(bal, bal_a - 100 + payout, "结算后余额对平");
    cleanup(&db).await;
}

/// 社交底座：观战进出与计数、房间频道消息发拉（升序 + 昵称回填 + 限量）、
/// 座位 join users 取昵称/头像/称号/余额。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn pvp_social_spectate_messages_and_profile() {
    let db = pool().await;
    apply_migrations(&db).await;
    cleanup(&db).await;
    let (a, bal_a) = mk_user(&db, 10_000).await;
    let (b, _) = mk_user(&db, 10_000).await;

    let mut tx = db.begin().await.unwrap();
    let (rid, ..) = super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, a, 100, "classic", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    // b 观战进房：不占座；计数 +1；可定位到该房
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::spectate_tx(&mut tx, rid, b)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        super::doudizhu_pvp_repo::spectate_room_db(&db, b)
            .await
            .unwrap(),
        Some(rid)
    );
    let room = super::doudizhu_pvp_view::load_room(&db, rid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(room.spectators, 1);
    assert!(!super::doudizhu_pvp_repo::is_seat_db(&db, rid, b)
        .await
        .unwrap());

    // 座位资料 join：昵称非空、余额来自 users
    let seat0 = &room.seats[0];
    assert_eq!(seat0.1, a);
    assert!(seat0.4.as_ref().is_some_and(|n| !n.is_empty()));
    assert_eq!(seat0.7, Some(bal_a));

    // 消息：a（座位）与 b（观战）各发一条
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::post_message_tx(&mut tx, rid, a, "你好")
        .await
        .unwrap();
    super::doudizhu_pvp_repo::post_message_tx(&mut tx, rid, b, "围观中")
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let msgs = super::doudizhu_pvp_repo::list_messages_db(&db, rid, 50)
        .await
        .unwrap();
    assert_eq!(msgs.len(), 2);
    // 升序：先 a 后 b；昵称回填非空
    assert_eq!(msgs[0].1, a);
    assert_eq!(msgs[1].1, b);
    assert!(!msgs[0].2.is_empty() && !msgs[1].2.is_empty());
    // limit=1 只回最近一条（升序末尾）
    let last = super::doudizhu_pvp_repo::list_messages_db(&db, rid, 1)
        .await
        .unwrap();
    assert_eq!(last.len(), 1);
    assert_eq!(last[0].1, b);

    // 退出观战：计数清零，定位不到
    super::doudizhu_pvp_repo::leave_spectate_db(&db, b)
        .await
        .unwrap();
    assert!(super::doudizhu_pvp_repo::spectate_room_db(&db, b)
        .await
        .unwrap()
        .is_none());
    let room = super::doudizhu_pvp_view::load_room(&db, rid)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(room.spectators, 0);
    cleanup(&db).await;
}

/// 观战读局：开局后观战者无本人镜像行，但能按所观房间取到权威活跃局
/// （HTTP 层再把手牌脱敏；这里验证 repo 回显口径与座位隔离）。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn pvp_spectator_reads_room_game() {
    let db = pool().await;
    apply_migrations(&db).await;
    cleanup(&db).await;
    let (a, _) = mk_user(&db, 10_000).await;
    let (b, _) = mk_user(&db, 10_000).await;
    let (c, _) = mk_user(&db, 10_000).await;
    let (d, _) = mk_user(&db, 10_000).await;

    let mut tx = db.begin().await.unwrap();
    let (rid, _, _) = super::doudizhu_pvp_repo::join_or_create_tx(
        &mut tx, a, 100, "classic", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    for uid in [b, c] {
        let mut tx = db.begin().await.unwrap();
        super::doudizhu_pvp_repo::join_or_create_tx(
            &mut tx, uid, 100, "classic", "public",
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
    }
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::deal_room_tx(&mut tx, rid)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // d 入座观战
    let mut tx = db.begin().await.unwrap();
    super::doudizhu_pvp_repo::spectate_tx(&mut tx, rid, d)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // 观战者没有本人镜像活跃行
    assert!(super::doudizhu_repo::load_active(&db, d)
        .await
        .unwrap()
        .is_none());
    // 按所观房间能取到一行权威活跃局
    let game = super::doudizhu_repo::load_room_game_db(&db, rid)
        .await
        .unwrap();
    let game = game.expect("观战房应有活跃局可回显");
    assert_eq!(game.room_id, Some(rid));
    assert!(!super::doudizhu_pvp_repo::is_seat_db(&db, rid, d)
        .await
        .unwrap());
    cleanup(&db).await;
}

/// 可见性：private 房不进公开匹配列表；public 房进。
#[tokio::test]
#[ignore = "DB 级：CI 无库；本地 cargo test -p flux-api doudizhu_db -- --ignored"]
async fn pvp_visibility_list_filter() {
    let db = pool().await;
    apply_migrations(&db).await;
    cleanup(&db).await;
    let (a, _) = mk_user(&db, 10_000).await;
    let (c, _) = mk_user(&db, 10_000).await;

    let mut tx = db.begin().await.unwrap();
    let rid_pub = super::doudizhu_pvp_repo::create_room_tx(
        &mut tx, a, 100, "classic", "public",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut tx = db.begin().await.unwrap();
    let rid_priv = super::doudizhu_pvp_repo::create_room_tx(
        &mut tx, c, 100, "classic", "private",
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    let list = super::doudizhu_pvp_view::list_waiting(&db, "classic")
        .await
        .unwrap();
    let vis: Vec<(i64, String)> =
        list.iter().map(|r| (r.id, r.visibility.clone())).collect();
    assert!(vis.contains(&(rid_pub, "public".into())));
    assert!(
        !vis.iter().any(|(id, _)| *id == rid_priv),
        "private 房不得进公开列表"
    );

    let priv_room = super::doudizhu_pvp_view::load_room(&db, rid_priv)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(priv_room.visibility, "private");
    cleanup(&db).await;
}
