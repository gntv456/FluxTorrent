//! hour_jobs：小时批整段任务清单（从 run.rs 拆出——调度循环本体留在
//! run.rs，本文件只列「每小时跑哪些 job」）。引用的各 job 入口经
//! jobs/mod.rs 的 glob 再导出（`use super::*`），与本批拆出前同源。

use super::run::run_sharded;
use super::*;
use sqlx::PgPool;

pub(super) async fn hour_jobs(
    db: PgPool,
    mut redis: redis::aio::ConnectionManager,
    shard: Option<Vec<String>>,
) {
    // 僵尸阈值同 tick 现算（seeding_reward 也要用）
    let stale_secs = stale_peer_threshold_secs(&db).await;
    run_sharded(
        &db,
        &shard,
        "job:seeding_reward",
        seeding_reward(&db, stale_secs),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:purge_old_login_events",
        purge_old_login_events(&db),
    )
    .await;
    run_sharded(&db, &shard, "job:ratio_watch", ratio_watch(&db)).await;
    run_sharded(&db, &shard, "job:dormant_mark", dormant_mark(&db)).await;
    run_sharded(&db, &shard, "job:request_expire", request_expire(&db)).await;
    run_sharded(&db, &shard, "job:wishlist_notify", wishlist_notify(&db)).await;
    run_sharded(&db, &shard, "job:highspeed_tag", highspeed_tag(&db)).await;
    run_sharded(
        &db,
        &shard,
        "job:resurrection_settle",
        resurrection_settle(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:social_team_settle",
        social_team_settle(&db),
    )
    .await;
    // 失败判定必须排在成功结算之后：恰好在期限内达标的队伍应算成功
    run_sharded(
        &db,
        &shard,
        "job:social_team_expire",
        social_team_expire(&db),
    )
    .await;
    // 0382 勋章自动成就：threshold 周期重算 + 回填推进。
    // 两条都幂等可重跑（已授予即跳过、回填靠「扫到哪」续跑），
    // 故挂 hourly 而非 daily——threshold 的周期口径含日/周，
    // 一小时一轮才不会出现「周期内最后一天才达标的要等 24h」。
    // 0382 做种事件桥：把「本小时新增做种时长」记成可计数事件。
    // 放在 hourly 而非 announce 热路径——announce 每秒数千次，
    // 在那里插 INSERT 是拿全站最热的写表换一个低频事件源。
    run_sharded(
        &db,
        &shard,
        "job:medal_seeding_bridge",
        crate::jobs::medal_achv::record_seeding_hours(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:medal_threshold",
        crate::jobs::medal_achv::threshold_sweep(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:medal_backfill",
        crate::jobs::medal_achv::medal_backfill(&db),
    )
    .await;
    // 0383 商城履约自愈：把卡在 frozen 且过了宽限的订单推进到终态
    // 「钱不会被无限期占着」这条不变量的
    // 唯一保证——没有它，三段式只是把「扣款后崩掉」换成了
    // 「冻结后崩掉」。
    run_sharded(
        &db,
        &shard,
        "job:shop_fulfill_recover",
        crate::jobs::shop_fulfill::recover(&db),
    )
    .await;
    // 绩效月末结算挂 hourly（0106）：daily 首轮被跳过、重启后要等 24h
    run_sharded(&db, &shard, "job:jixiao_settle", jixiao_settle(&db)).await;
    run_sharded(&db, &shard, "job:preserve_settle", preserve_settle(&db)).await;
    run_sharded(&db, &shard, "job:funding_settle", funding_settle(&db)).await;
    run_sharded(&db, &shard, "job:refundable_settle", refundable_settle(&db))
        .await;
    run_sharded(&db, &shard, "job:achievement_grant", achievement_grant(&db))
        .await;
    // 每日做种满 6h 发口粮券（幂等靠发放表 PK）
    run_sharded(&db, &shard, "job:game_coupons", grant_food_coupons(&db)).await;
    // 字幕三扫（0148/0149）+ 卫生清理簇（邀请回收/凭证/日志/勋章）
    run_sharded(&db, &shard, "job:subreq_sweep", subreq_sweep(&db)).await;
    run_sharded(&db, &shard, "job:subawards", subawards_build(&db)).await;
    run_sharded(&db, &shard, "job:subcert_sweep", subcert_sweep(&db)).await;
    run_sharded(&db, &shard, "job:expire_medals", expire_medals(&db)).await;
    run_sharded(&db, &shard, "job:expire_invites", expire_invites(&db)).await;
    run_sharded(
        &db,
        &shard,
        "job:purge_expired_tokens",
        purge_expired_tokens(&db),
    )
    .await;
    run_sharded(
        &db,
        &shard,
        "job:purge_runtime_logs",
        purge_runtime_logs(&db),
    )
    .await;
    // 审计日志归档（0357）：小时内检查，Redis 键节流每日最多一批
    run_sharded(
        &db,
        &shard,
        "job:audit_retain",
        audit_retain_tick(&db, &redis),
    )
    .await;
    // DLQ 积压告警（只进不出等于变相丢计费）
    {
        let (db2, mut r) = (db.clone(), redis.clone());
        run_sharded(&db, &shard, "job:dlq_watch", async move {
            dlq_watch(&db2, &mut r).await
        })
        .await;
    }
}
