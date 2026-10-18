//! 插件运行时：能力句柄（PluginCtx）+ 运行开关缓存。
//!
//! 安全模型（M28 → 0367 批）：native 插件**不再持有 `&AppState`**，只拿
//! `PluginCtx`——DB 唯一入口 `db_exec` / `db_fetch_all` 先过 `sql_guard`
//! 结构校验，再 sqlx 参数化执行。插件能碰哪些表/列由 `DbCapability` 声明，
//! 越权即拒并记审计。这消解了「插件持完整连接池跑裸 SQL」的 CVE 类风险
//! （NexusPHP 19 个 CVE 的同款弱点）。
//!
//! 模块内容：
//! - `DbMode` / `DbCapability`：插件声明的数据库能力（契约，sql_guard 消费）；
//! - `PluginCtx`：分发时按插件能力构造，hook 的唯一下下文；
//! - `plugin_enabled` / `invalidate_plugin_cache`：运行开关（site_settings
//!   `plugin__{name}`，30s 进程内缓存；自 plugins.rs 迁出，调用方不变语义）。

use sqlx::postgres::{PgArguments, PgQueryResult, PgRow};
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 数据库能力模式。
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
pub enum DbMode {
    Read,
    Write,
}

/// 插件的数据库能力声明：允许碰某表的哪些列、读还是写、能否无界写。
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct DbCapability {
    pub table: String,
    pub columns: Vec<String>,
    pub mode: DbMode,
    /// 是否允许无 WHERE 的整表 UPDATE/DELETE（默认 false）。
    pub allow_unbounded: bool,
}

/// 插件查询的绑定值（避免泛型地狱，覆盖插件常用类型）。
#[derive(Clone, Debug)]
pub enum Bind {
    I64(i64),
    OptI64(Option<i64>),
    Str(String),
    OptStr(Option<String>),
    Bool(bool),
}

/// 插件能力句柄：hook 的唯一下下文。字段私有，插件只能经方法访问。
///
/// 注意：方法内部用 `Handle::current().block_on`（hook 在 `spawn_blocking`
/// 内运行，运行时上下文存在）——必须在插件 hook 内调用，不能在无运行时的
/// 裸线程里调。
pub struct PluginCtx<'a> {
    pool: &'a PgPool,
    caps: &'a [DbCapability],
    name: &'static str,
    /// DB 层隔离态（0378）。`role: None` = 降级到纯 sql_guard，
    /// 与 0367 同级别。构造时由调用方 `ensure_isolation` 预先备好——
    /// hook 是同步的（`block_on`），不能在里面 await 建角色。
    iso: &'a crate::plugin_sql::Isolation,
}

impl<'a> PluginCtx<'a> {
    pub fn new(
        pool: &'a PgPool,
        caps: &'a [DbCapability],
        name: &'static str,
        iso: &'a crate::plugin_sql::Isolation,
    ) -> Self {
        Self {
            pool,
            caps,
            name,
            iso,
        }
    }

    /// 执行写/读语句（无行返回）。先过 sql_guard，再参数化执行。
    pub fn db_exec(
        &self,
        sql: &str,
        binds: &[Bind],
    ) -> DomainResult<PgQueryResult> {
        self.guard(sql)?;
        let (pool, iso, sql, binds) = (self.pool, self.iso, sql, binds);
        self.block(async move {
            crate::plugin_sql::exec_isolated(pool, iso, sql, binds).await
        })
    }

    /// 查询多行（行读取用 `sqlx::Row::get`，列名即声明内的列）。
    pub fn db_fetch_all(
        &self,
        sql: &str,
        binds: &[Bind],
    ) -> DomainResult<Vec<PgRow>> {
        self.guard(sql)?;
        let (pool, iso, sql, binds) = (self.pool, self.iso, sql, binds);
        self.block(async move {
            crate::plugin_sql::fetch_all_isolated(pool, iso, sql, binds).await
        })
    }

    /// 单值 i64 查询便捷方法（EXISTS/count 场景）。
    pub fn db_fetch_scalar_i64(
        &self,
        sql: &str,
        binds: &[Bind],
    ) -> DomainResult<Option<i64>> {
        self.guard(sql)?;
        let (pool, iso, sql, binds) = (self.pool, self.iso, sql, binds);
        self.block(async move {
            crate::plugin_sql::fetch_scalar_i64_isolated(pool, iso, sql, binds)
                .await
        })
    }

    fn guard(&self, sql: &str) -> DomainResult<()> {
        if let Err(e) = crate::sql_guard::validate(sql, self.caps) {
            self.audit_reject(&e.to_string());
            return Err(e);
        }
        Ok(())
    }

    /// 在 hook 的同步上下文里驱动一个 future 到完成。
    ///
    /// 泛型覆盖两种 Output：`Result<T, sqlx::Error>`（裸 pool 路径）与
    /// `DomainResult<T>`（隔离执行路径，内部已把错误映射成 DomainError）。
    /// 两者都归一到 `DomainResult<T>` 返回。
    fn block<F, T>(&self, fut: F) -> DomainResult<T>
    where
        F: std::future::Future<Output = DomainResult<T>>,
    {
        tokio::runtime::Handle::current()
            .block_on(fut)
            .map_err(|e| DomainError::Internal(e.into()))
    }

    pub fn log_info(&self, msg: &str) {
        tracing::info!(plugin = self.name, "{msg}");
    }

    #[allow(dead_code)]
    pub fn log_warn(&self, msg: &str) {
        tracing::warn!(plugin = self.name, "{msg}");
    }

    /// 校验拒绝审计：只记原因（不记 SQL 原文，避免把绑定数据写进审计）。
    pub fn audit_reject(&self, reason: &str) {
        let pool = self.pool;
        let name = self.name;
        let reason: String = reason.chars().take(200).collect();
        let _ = self.block(async move {
            sqlx::query(
                "INSERT INTO audit_log (id, actor_id, action, ref) \
                 VALUES (nextval('audit_log_id_seq'), NULL, $1, $2::jsonb)",
            )
            .bind(format!("plugin.{name}.sql_reject"))
            .bind(serde_json::json!({ "reason": reason }).to_string())
            .execute(pool)
            .await
            .map_err(|e| DomainError::Internal(e.into()))
        });
    }
}

/// 把 `Bind` 绑进 sqlx 查询（Query / QueryScalar 两种形态统一接口）。
/// 字符串克隆为自有值再绑——避免绑定值借短生命周期 `Bind` 而与 SQL 的
/// `'q` 冲突（插件 SQL 量小，克隆成本可忽略）。
trait BindVal: Sized {
    fn bind_val(self, b: &Bind) -> Self;
}

impl<'q> BindVal for sqlx::query::Query<'q, sqlx::Postgres, PgArguments> {
    fn bind_val(self, b: &Bind) -> Self {
        match b {
            Bind::I64(v) => self.bind(*v),
            Bind::OptI64(v) => self.bind(*v),
            Bind::Str(s) => self.bind(s.clone()),
            Bind::OptStr(s) => self.bind(s.clone()),
            Bind::Bool(v) => self.bind(*v),
        }
    }
}

impl<'q> BindVal
    for sqlx::query::QueryScalar<'q, sqlx::Postgres, i64, PgArguments>
{
    fn bind_val(self, b: &Bind) -> Self {
        match b {
            Bind::I64(v) => self.bind(*v),
            Bind::OptI64(v) => self.bind(*v),
            Bind::Str(s) => self.bind(s.clone()),
            Bind::OptStr(s) => self.bind(s.clone()),
            Bind::Bool(v) => self.bind(*v),
        }
    }
}

// ============ 运行开关（自 plugins.rs 迁出，语义不变） ============

/// 插件运行时开关（0214，五路方案 P1-3.5）：site_settings 的 `plugin__{name}`
/// 键（yes/no，缺省 yes）。分发处同步询问——站长可一键停用某插件；
/// 键值热读有查询成本，用 30s 进程内缓存摊薄。
/// 开关缓存（模块级静态，0224 G30）：plugin_enabled 读；
/// invalidate_plugin_cache 清（cfg:ver modules 域命中时同步失效）。
type PluginCache = std::sync::Mutex<
    std::collections::HashMap<String, (std::time::Instant, bool)>,
>;

static PLUGIN_CACHE: tokio::sync::OnceCell<PluginCache> =
    tokio::sync::OnceCell::const_new();

/// 缓存整体失效（未初始化时跳过——首次读本身就是全量新值）。
pub fn invalidate_plugin_cache() {
    if let Some(c) = PLUGIN_CACHE.get() {
        if let Ok(mut g) = c.lock() {
            g.clear();
        }
    }
}

pub async fn plugin_enabled(db: &sqlx::PgPool, name: &str) -> bool {
    let cache = PLUGIN_CACHE
        .get_or_init(|| async { PluginCache::default() })
        .await;
    if let Ok(g) = cache.lock() {
        if let Some((at, v)) = g.get(name) {
            if at.elapsed() < std::time::Duration::from_secs(30) {
                return *v;
            }
        }
    }
    let v: Option<String> =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
            .bind(format!("plugin__{name}"))
            .fetch_optional(db)
            .await
            .ok()
            .flatten();

    let v = !matches!(v.as_deref(), Some("no"));
    if let Ok(mut g) = cache.lock() {
        g.insert(name.to_string(), (std::time::Instant::now(), v));
    }
    v
}
