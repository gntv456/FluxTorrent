//! tracing 订阅器初始化（2026-11-26 从 main.rs 拆出，门禁拆件还债）。
//!
//! 口径不变，只搬家：
//! - fmt 层保留 stdout 输出；runtime_log 层把 WARN+ 也写进库（后台「运行日志」页）
//! - FLUX_LOG_JSON=1/true（任务3）：fmt 层切 JSON 输出（容器/日志采集器友好，
//!   flatten_event 把事件字段提到顶层）；缺省保持人读格式不变
//! - runtime_log 落库层是独立 Layer（只消费字段，与 fmt 格式无关），WARN+
//!   落库不受影响

/// 进程日志初始化：main() 开头调用一次。
pub fn init_tracing() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let log_json = matches!(
        std::env::var("FLUX_LOG_JSON")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true"
    );
    if log_json {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info,sqlx=warn".into()),
            )
            .with(tracing_subscriber::fmt::layer().json().flatten_event(true))
            .with(crate::runtime_log::layer("api"))
            .init();
    } else {
        tracing_subscriber::registry()
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info,sqlx=warn".into()),
            )
            .with(tracing_subscriber::fmt::layer())
            .with(crate::runtime_log::layer("api"))
            .init();
    }
}
