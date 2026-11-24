//! 池抽玩法 DB 级测试：直调 `load_pool`（真实读取链路），锁「库里实际播种的
//! 池过 EV 闸 + 憨憨结构池形状」——防「库被后台改宽而代码单测还绿」。
//!
//! 跑法：本地有 flux-postgres 时
//! `cargo test -p flux-api pool_db -- --ignored`；CI 无库自动跳过。

use crate::games::mole_db_pool as h;
use crate::games_http::pool::load_pool;

/// 连库夹具 = `games::mole_db_pool`（layer_guard 对本目录新文件
/// 零容忍 `sqlx` 字样，连接代码不放这里）。

/// 当前生效的 wheel 池必须能过 `load_pool` 的关闸校验（EV<1 等全部结构项）。
/// 换池（如 0519 憨憨结构）后这里就是第一道真实读取链路的闸。
#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn active_wheel_pool_passes_the_gate() {
    let db = h().await;
    let pool = load_pool(&db, "wheel")
        .await
        .expect("当前生效的 wheel 池必须可加载（配置坏=玩法拒绝服务）");
    assert!(pool.ticket > 0);
    assert!(!pool.entries.is_empty());
    let ev = games_pool_ev(&pool, pool.ticket);
    assert!(
        ev < 1.0,
        "生效 wheel 池 EV {ev} 必须 < 1（回收口纪律，库内播种也要管）"
    );
}

/// 0519 憨憨结构池的形状锁定：10 档、保底主导、EV < 0.5（重回收口径）。
/// 旧池 wheel_default 被禁用后，`load_pool("wheel")` 读到的就是这一池。
#[tokio::test]
#[ignore = "DB 级测试：CI 无库；本地 cargo test ... -- --ignored 实连跑"]
async fn hhan_wheel_pool_shape_is_locked() {
    let db = h().await;
    let pool = load_pool(&db, "wheel").await.expect("load wheel pool");
    // 形状：10 档（憨憨原表 10 奖品的映射）
    assert_eq!(pool.entries.len(), 10, "憨憨结构池应为 10 档");
    // 保底主导：最大权重档是「谢谢参与」（0.15x 安慰）
    let top = pool
        .entries
        .iter()
        .max_by_key(|e| e.weight)
        .expect("non-empty");
    assert_eq!(top.label, "谢谢参与");
    let total: u64 = pool.entries.iter().map(|e| u64::from(e.weight)).sum();
    let comfort = u64::from(top.weight);
    assert!(
        comfort * 100 > total * 90,
        "保底档应 > 90%（憨憨形状），实得 {}‰",
        comfort * 1000 / total
    );
    // 重回收口径：EV < 0.5（对比旧池 0.85）
    let ev = games_pool_ev(&pool, pool.ticket);
    assert!(ev < 0.5, "憨憨结构池 EV {ev} 应 < 0.5（重回收口径）");
    // 物品位在池里真实可折算（anchor > 0），发不出时走 FALLBACK 而非空头
    let items = pool
        .entries
        .iter()
        .filter(|e| matches!(e.kind, crate::games::EntryKind::Item { .. }))
        .count();
    assert!(items >= 4, "物品档应至少 4 个（补签卡/抽卡券/银行券×1/×2）");
}

/// 与 games::jgg::pool_ev 同口径的本地helper（pool_ev 是 pub 的，直接用）。
fn games_pool_ev(pool: &crate::games_http::pool::Pool, ticket: i64) -> f64 {
    crate::games::pool_ev(&pool.entries, ticket)
}
