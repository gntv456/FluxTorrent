//! 运行日志出口（0218 G6）：WARN+ 事件批量落 runtime_logs，供站长排查站点报错。
//! ⚠️ 与 apps/worker/src/runtime_log.rs 同源（两 crate 不共享依赖，改动需同步）。
//!
//! 设计：Layer 只做级别筛选与消息格式化，随后 try_send 进有界通道；
//! 落库由独立任务批量写（同批连续重复折叠成一行 + repeat 计数）。
//! 因此日志永不阻塞业务线程；DB 不可用时静默丢弃并在 stderr 提示，恢复后自愈。
//! 级别门槛：FLUX_RTLOG_LEVEL=off|error|warn|info|debug（缺省 warn）。

use sqlx::PgPool;
use std::sync::OnceLock;
use tokio::sync::mpsc;

#[derive(Clone)]
struct Entry {
    ts: chrono::DateTime<chrono::Utc>,
    level: &'static str,
    source: &'static str,
    target: String,
    message: String,
}

static TX: OnceLock<mpsc::Sender<Entry>> = OnceLock::new();

/// 采集层：在 subscriber 组装时挂上（此刻尚无 DB 连接，先只入通道）。
pub fn layer(source: &'static str) -> DbLayer {
    DbLayer {
        max: level_from_env(),
        source,
    }
}

/// 挂上 DB 落库任务（进程内一次；pool 建好后调用）。
pub fn attach(db: PgPool) {
    if TX.get().is_some() {
        return;
    }
    let (tx, rx) = mpsc::channel(2048);
    if TX.set(tx).is_err() {
        return;
    }
    tokio::spawn(writer(db, rx));
}

pub struct DbLayer {
    max: tracing::level_filters::LevelFilter,
    source: &'static str,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for DbLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if self.max < *event.metadata().level() {
            return;
        }
        let mut v = MsgVisitor::default();
        event.record(&mut v);
        let entry = Entry {
            ts: chrono::Utc::now(),
            level: level_name(event.metadata().level()),
            source: self.source,
            target: event.metadata().target().to_string(),
            message: v.finish(),
        };
        if let Some(tx) = TX.get() {
            let _ = tx.try_send(entry);
        }
    }
}

#[derive(Default)]
struct MsgVisitor {
    msg: String,
    fields: Vec<String>,
}

impl MsgVisitor {
    /// 消息 = message 字段 + 结构化字段（`k=v`）——排查要的正是 job 名/错误详情这些。
    fn finish(mut self) -> String {
        if self.fields.is_empty() {
            return std::mem::take(&mut self.msg);
        }
        format!("{} [{}]", self.msg, self.fields.join(" "))
    }
}

impl tracing::field::Visit for MsgVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.msg = value.to_string();
        } else {
            self.fields.push(format!("{}={}", field.name(), value));
        }
    }

    fn record_debug(
        &mut self,
        field: &tracing::field::Field,
        value: &dyn std::fmt::Debug,
    ) {
        if field.name() == "message" {
            // fmt 层把 message 以 Debug 记录（带引号），去掉引号还原成原句
            let raw = format!("{value:?}");
            self.msg = raw.trim_matches('"').to_string();
        } else {
            self.fields.push(format!("{}={:?}", field.name(), value));
        }
    }
}

fn level_name(l: &tracing::Level) -> &'static str {
    match *l {
        tracing::Level::ERROR => "ERROR",
        tracing::Level::WARN => "WARN",
        tracing::Level::INFO => "INFO",
        tracing::Level::DEBUG => "DEBUG",
        _ => "TRACE",
    }
}

fn level_from_env() -> tracing::level_filters::LevelFilter {
    use tracing::level_filters::LevelFilter;
    match std::env::var("FLUX_RTLOG_LEVEL")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "off" => LevelFilter::OFF,
        "error" => LevelFilter::ERROR,
        "info" => LevelFilter::INFO,
        "debug" => LevelFilter::DEBUG,
        _ => LevelFilter::WARN,
    }
}

/// 批量写：recv_many 一次最多 64 条，同批连续重复折叠（ts 取末次、repeat 累加）。
async fn writer(db: PgPool, mut rx: mpsc::Receiver<Entry>) {
    let mut batch: Vec<Entry> = Vec::with_capacity(64);
    while rx.recv_many(&mut batch, 64).await > 0 {
        while batch.len() < 64 {
            match rx.try_recv() {
                Ok(e) => batch.push(e),
                Err(_) => break,
            }
        }
        if let Err(e) = flush(&db, &batch).await {
            // 不能走 tracing（会自反馈）：直落 stderr
            eprintln!("[runtime_log] 落库失败（丢弃 {} 条）: {e}", batch.len());
        }
        batch.clear();
    }
}

/// 实例标识（0224 G30）：多副本下区分日志来源机器；不设 FLUX_INSTANCE_ID
/// 时取容器 hostname，单实例部署通常不配置——列值为 ''，行为与以前一致。
fn instance_id() -> String {
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

async fn flush(db: &PgPool, batch: &[Entry]) -> Result<(), sqlx::Error> {
    let instance = instance_id();
    // 连续重复折叠（ts 取末次、repeat 累加）——与 worker 侧同逻辑
    // 连续重复折叠（ts 取末次、repeat 累加）——与 worker 侧同逻辑。
    // 聚合元组持克隆而非引用：末次 ts 要覆写，引用不可变。
    let mut agg: Vec<(Entry, i32)> = Vec::with_capacity(batch.len());
    for e in batch {
        if let Some((last, n)) = agg.last_mut() {
            if last.level == e.level
                && last.target == e.target
                && last.message == e.message
            {
                last.ts = e.ts;
                *n += 1;
                continue;
            }
        }
        agg.push((e.clone(), 1));
    }
    if agg.is_empty() {
        return Ok(());
    }
    // 手工拼 VALUES（push_values 闭包与 instance String 借用生命周期冲突）
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO runtime_logs \
         (ts, level, source, target, message, repeat, instance) VALUES ",
    );
    for (i, (e, n)) in agg.iter().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push("(");
        qb.push_bind(e.ts);
        qb.push(", ");
        qb.push_bind(e.level);
        qb.push(", ");
        qb.push_bind(e.source);
        qb.push(", ");
        qb.push_bind(e.target.as_str());
        qb.push(", ");
        qb.push_bind(e.message.as_str());
        qb.push(", ");
        qb.push_bind(*n);
        qb.push(", ");
        qb.push_bind(instance.as_str());
        qb.push(")");
    }
    qb.build().execute(db).await.map(|_| ())
}
