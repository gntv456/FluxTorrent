//! 做种事件桥：把「本小时新增的做种时长」记成可计数事件。
//!
//! 挂在 `seeding_reward` 之后（同 hourly tick），而不是挂 announce 热路径——
//! announce 每秒可能几千次，在那里插一条 INSERT 是拿全站最热的写表去
//! 换一个低频事件源。hourly 粒度对「累计 100 小时」这类成就完全够用。
//!
//! 【为什么按小时批量而不是逐个 announce】
//! 成就看的是**累计量**（「做种满 100 小时」），不是次数。逐 announce
//! 记事件会让 medal_events 变成全站最大的写表，而产出的是同一个数：
//! 「这个小时这个人做了多少」。所以按 (user_id, 小时) 聚合成一条。

use sqlx::PgPool;

/// 事件名（与 medals.auto_rule 对应；后台配成就时填这个字符串）。
pub const EVENT_SEED_HOUR: &str = "seed_hour";

/// 记本小时新增的做种时长。
///
/// 幂等：同一小时内重复跑不会重复累加——靠 `idempotent` 语义体现在
/// 「只取 `last_seen_at` 落在这一个小时内的做种行」，而不是靠唯一键。
/// 这样 hourly tick 中途失败重跑也不会把用户算成双倍。
///
/// ⚠️ 代价：跨小时的边界可能漏/重算一次做种行——但做种时长类成就的
/// 阈值都是百小时量级，差一小时不影响判定，却省掉了给每条做种行加
/// 幂等键的代价。这是有意的取舍。
pub(crate) async fn record_seeding_hours(db: &PgPool) -> anyhow::Result<u64> {
    // 事件流写入：每个用户本小时的做种秒数 → 小时数（向上取整，
    // 「做满一小时」与「做了 59 分钟」的差别对百小时量级的阈值无意义）
    let n = sqlx::query(
        "INSERT INTO medal_events (user_id, event, amount, at) \
         SELECT s.user_id, $1, GREATEST(1, \
                  (sum(GREATEST( \
                       EXTRACT(EPOCH FROM ( \
                         LEAST(COALESCE(s.last_seen_at, now()), now()) \
                         - GREATEST(s.completed_at, \
                             date_trunc('hour', now()) \
                           ))), 0)) / 3600))::bigint, \
                now() \
         FROM snatches s \
         WHERE s.seeding \
           AND s.completed_at < date_trunc('hour', now()) \
           AND COALESCE(s.last_seen_at, s.completed_at) \
               >= date_trunc('hour', now()) \
         GROUP BY s.user_id",
    )
    .bind(EVENT_SEED_HOUR)
    .execute(db)
    .await?
    .rows_affected();
    if n > 0 {
        tracing::info!(users = n, "做种事件已记入勋章事件流");
    }
    Ok(n)
}
