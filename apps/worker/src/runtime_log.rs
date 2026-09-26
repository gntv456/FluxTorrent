//! 运行日志出口（0218 G6）：WARN+ 事件批量落 runtime_logs，供站长排查站点报错。
//! ⚠️ 与 apps/api/src/runtime_log.rs 同源（两 crate 不共享依赖，改动需同步）。
//!
//! 设计：Layer 只做级别筛选与消息格式化，随后 try_send 进有界通道；
//! 落库由独立任务批量写（同批连续重复折叠成一行 + repeat 计数）。
//! 因此日志永不阻塞业务线程；DB 不可用时静默丢弃并在 stderr 提示，恢复后自愈。
//! 级别门槛：FLUX_RTLOG_LEVEL=off|error|warn|info|debug（缺省 warn）。

use sqlx::PgPool;
use std::sync::OnceLock;
use tokio::sync::mpsc;

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

/// rows: (ts, level, source, target, message, repeat)
type Row = (
    chrono::DateTime<chrono::Utc>,
    &'static str,
    &'static str,
    String,
    String,
    i32,
);

async fn flush(db: &PgPool, batch: &[Entry]) -> Result<(), sqlx::Error> {
    let mut rows: Vec<Row> = Vec::with_capacity(batch.len());
    for e in batch {
        if let Some(last) = rows.last_mut() {
            if last.1 == e.level && last.3 == e.target && last.4 == e.message {
                last.0 = e.ts;
                last.5 += 1;
                continue;
            }
        }
        rows.push((
            e.ts,
            e.level,
            e.source,
            e.target.clone(),
            e.message.clone(),
            1,
        ));
    }
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO runtime_logs \
         (ts, level, source, target, message, repeat) ",
    );
    qb.push_values(rows, |mut b, r| {
        b.push_bind(r.0)
            .push_bind(r.1)
            .push_bind(r.2)
            .push_bind(r.3)
            .push_bind(r.4)
            .push_bind(r.5);
    });
    qb.build().execute(db).await.map(|_| ())
}
