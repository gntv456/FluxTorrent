//! Redis 连接管理器的引导装配（从 main.rs 拆出，同受 300 行门禁约束）。
//!
//! 两处默认值都必须显式覆盖，都有 2026-10-10 的实测事故背书：
//!
//! ① `response_timeout`：redis 0.27.6 的 `ConnectionManagerConfig::default()`
//!    是 `None`（src/aio/connection_manager.rs:73），含义是「管理器正在重连时，
//!    任何命令**永远挂着**」。所以 Redis 一不可达，tracker 就变成
//!    「容器 running、端口不监听、`docker logs` 零行、客户端拿
//!    RemoteDisconnected」的僵尸。设 2s 上限后命令返回错误，热路径按既有
//!    设计降级（限流走本地兜底、计费事件进重投队列）——是降级，不是停摆。
//!
//! ② `number_of_retries` 与「引导超时」必须分两条轴。这个参数同时管引导连接与
//!    运行期每次断线后的重连次数：设成小数字（如 3）会让一次稍长的抖动就把
//!    管理器**永久弃用**，之后只能靠 20k 上限的重投队列兜，队列满就真丢账。
//!    所以运行期用 `usize::MAX`（配 `max_delay` 封顶退避，会自愈），
//!    而引导期用 `timeout` 包住首次获取——连不上就带着原因响亮失败退出，
//!    交给 `restart: unless-stopped` 重试，绝不返回一个挂死的句柄继续跑。

use std::time::Duration;

use redis::aio::{ConnectionManager, ConnectionManagerConfig};

/// 引导预算（秒）。默认 15s：够跑完一次 TCP + AUTH，又不至于让部署卡住。
fn boot_budget() -> Duration {
    let secs = crate::http_track::helpers::env_i64(
        "FLUX_TRACKER_REDIS_BOOT_BUDGET_SECS",
        15,
    )
    .clamp(2, 120) as u64;
    Duration::from_secs(secs)
}

pub(crate) async fn connect(url: &str) -> anyhow::Result<ConnectionManager> {
    let budget = boot_budget();
    let cfg = ConnectionManagerConfig::new()
        .set_number_of_retries(usize::MAX)
        .set_exponent_base(2)
        .set_factor(100)
        .set_max_delay(2)
        .set_response_timeout(Duration::from_secs(2));
    let client = redis::Client::open(url)?;
    match tokio::time::timeout(
        budget,
        client.get_connection_manager_with_config(cfg),
    )
    .await
    {
        Ok(Ok(m)) => Ok(m),
        Ok(Err(e)) => Err(anyhow::anyhow!("Redis 不可用（{url}）：{e}")),
        Err(_) => Err(anyhow::anyhow!(
            "Redis 在 {url} 上 {}s 内连不上，启动中止",
            budget.as_secs()
        )),
    }
}
