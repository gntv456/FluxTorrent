//! 停机协调（0224 G30）：SIGTERM/SIGINT 优雅停机 + 手动任务句柄收口。
//!
//! 现状缺口（附录 D 档一）：worker 是裸 tokio 永续循环，`docker stop` 默认 10s
//! 后 SIGKILL——在跑任务被硬掐，LB/编排拿不到排空窗口。
//! 设计：全局停机位 + 手动任务 JoinSet。收到信号后：不再认领新任务（select!
//! 分支短路）、已 spawn 的手动任务给 60s 收尾窗口（超时放弃，advisory 锁随
//! 连接关闭释放、被掐任务走既有 30 分钟中断回收 + 各 job 幂等键兜底）。
//! 定时任务本身不等待：它们在 tick 分支内串行 await，停机即停取新 tick。

use std::sync::atomic::{AtomicBool, Ordering};

/// 实例标识（0224 G30）：与 runtime_log 的 instance 同口径——FLUX_INSTANCE_ID
/// 优先、缺省回落容器 hostname；两处共用一份实现避免漂移。
pub(crate) fn instance_tag() -> String {
    if let Ok(v) = std::env::var("FLUX_INSTANCE_ID") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return v;
        }
    }
    std::fs::read_to_string("/etc/hostname")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// 停机位：置 true 后 poll_manual_triggers 不再认领、run_all 循环退出。
pub(crate) static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

pub(crate) fn shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::Relaxed)
}

/// 手动任务句柄收口：JoinSet 包装，停机时 join_all 限时等待。
pub(crate) struct TaskSet {
    inner: tokio::task::JoinSet<()>,
}

impl TaskSet {
    pub(crate) fn new() -> Self {
        Self {
            inner: tokio::task::JoinSet::new(),
        }
    }

    /// spawn 后立即收割已完成任务（防长跑进程句柄堆积）。
    pub(crate) fn spawn<F>(&mut self, fut: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        while self.inner.try_join_next().is_some() {}
        self.inner.spawn(fut);
    }

    /// 停机排空：等所有在跑手动任务完成，上限 60s。
    pub(crate) async fn drain(&mut self) {
        let _ =
            tokio::time::timeout(std::time::Duration::from_secs(60), async {
                while self.inner.join_next().await.is_some() {}
            })
            .await;
    }
}

/// 安装 SIGTERM/SIGINT 监听（unix；windows 下无此信号语义，跳过）。
pub(crate) fn install_signal_handler() {
    #[cfg(unix)]
    {
        tokio::spawn(async {
            use tokio::signal::unix::{signal, SignalKind};
            let mut term = match signal(SignalKind::terminate()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(?e, "SIGTERM 监听安装失败（停机走硬切）");
                    return;
                }
            };
            let mut int = match signal(SignalKind::interrupt()) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(?e, "SIGINT 监听安装失败（停机走硬切）");
                    return;
                }
            };
            tokio::select! {
                _ = term.recv() => {}
                _ = int.recv() => {}
            }
            tracing::info!(
                "收到停机信号：停止认领新任务，等待在跑任务收尾（≤60s）"
            );
            SHUTTING_DOWN.store(true, Ordering::Relaxed);
        });
    }
    #[cfg(not(unix))]
    {
        // Windows 本地开发无 SIGTERM；Ctrl+C 走 ctrl_c 监听
        tokio::spawn(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("收到 Ctrl+C：停止认领新任务");
            SHUTTING_DOWN.store(true, Ordering::Relaxed);
        });
    }
}
