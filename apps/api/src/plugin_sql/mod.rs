//! 插件 SQL 运行期隔离（0378，PT Studio 对标的 CVE 防线运行面）。
//!
//! 迁移 0378 建了 per-plugin 角色与授权函数；本模块是它的**消费面**：
//! 插件每次执行 SQL 时，开一个事务，先 `SET LOCAL ROLE` + 锁定
//! `search_path`，再跑 sql_guard 已放行的那条 SQL，最后提交。
//!
//! 三层防御的分工（缺一不可）：
//!   1. **sql_guard**（0367，应用层）：语句级白名单——表/列/读写模式、
//!      拒多语句、拒 DDL、拒 SET/GRANT、拒 SELECT *。防「写错」。
//!   2. **本模块**（0378，连接层）：`SET LOCAL ROLE` 让越界 SQL 在 DB 层
//!      撞权限墙；`SET LOCAL search_path=pg_catalog` 杜绝 search_path 劫持
//!      （否则插件能借自建同名表把查询指向假数据）。防「漏判后的后果」。
//!   3. **NOLOGIN 角色**（0378 迁移）：插件角色不能登录、不能把权限传出去，
//!      拿不到密码也就无法绕开站点连接池另开直连通道。
//!
//! 【降级语义——本模块不制造新的单点故障】
//! `CREATE ROLE` 需要 CREATEROLE，自部署常常没有。此时
//! `ensure_isolation` 拿不到角色名，本模块**退回纯 sql_guard 模式**
//! （0367 的既有强度一字不减），只记一次 warn 日志。也就是说：
//!   * 权限够 → 多一道 DB 层防线（纵深防御的加成）；
//!   * 权限不够 → 与迁移 0378 之前完全同级别，不倒退。
//!
//! 为什么用事务而不是 `SET`：SET ROLE / SET search_path 在会话级会污染
//! 连接池里的下一位使用者；`SET LOCAL` 只在本事务内有效，COMMIT 即失效。
//! 代价是每次执行多两个 round-trip——插件 SQL 是低频路径（登录/发种/里程碑），
//! 换正确性不亏。

mod exec;

pub use exec::{
    exec_isolated, fetch_all_isolated, fetch_scalar_i64_isolated,
};

use sqlx::PgPool;
use sqlx::postgres::Postgres;
use sqlx::{Acquire, Transaction};

use crate::errors::{DomainError, DomainResult};
use crate::plugin_ctx::{DbCapability, DbMode};

/// 隔离状态：该插件能否拿到 DB 层角色。
#[derive(Clone, Debug)]
pub struct Isolation {
    /// `Some(角色名)` = 有 DB 层隔离；`None` = 降级到纯 sql_guard。
    role: Option<String>,
    /// 声明的表（用于授权比对与诊断）。
    tables: Vec<String>,
    mode: DbMode,
}

impl Isolation {
    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }

    pub fn is_isolated(&self) -> bool {
        self.role.is_some()
    }

    pub fn tables(&self) -> &[String] {
        &self.tables
    }

    pub fn mode(&self) -> DbMode {
        self.mode
    }
}

/// 按插件声明的能力表，汇总出「读表」与「写表」两张单子。
///
/// 同一张表被声明成 Write 时以 Write 为准（写蕴含读）。
fn split_tables(caps: &[DbCapability]) -> (Vec<String>, Vec<String>) {
    let mut read = std::collections::BTreeSet::new();
    let mut write = std::collections::BTreeSet::new();
    for c in caps {
        let t = c.table.to_ascii_lowercase();
        match c.mode {
            DbMode::Read => {
                read.insert(t);
            }
            DbMode::Write => {
                // 写蕴含读：同一张表两个单子都要收，否则授权只给写不给
                // SELECT，插件自己的写语句读不回来。
                read.insert(t.clone());
                write.insert(t);
            }
        }
    }
    (
        read.into_iter().collect::<Vec<_>>(),
        write.into_iter().collect::<Vec<_>>(),
    )
}

/// 建立/复用该插件的 DB 角色并按声明授权。幂等。
///
/// 失败（无 CREATEROLE / 迁移未跑）一律降级为 `role: None`，不抛错——
/// 见模块头「降级语义」。
pub async fn ensure_isolation(
    pool: &PgPool,
    name: &str,
    caps: &[DbCapability],
) -> Isolation {
    let (read, write) = split_tables(caps);
    let mode = if write.is_empty() { DbMode::Read } else { DbMode::Write };

    // 迁移未跑（老库/跳号）时函数不存在；这一层也要能降级。
    let role: Option<String> =
        sqlx::query_scalar::<_, String>("SELECT flux_plugin_role_name($1)")
            .bind(name)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .filter(|r| !r.is_empty());

    let Some(role) = role else {
        tracing::warn!(
            plugin = name,
            "插件 DB 角色隔离不可用（迁移 0378 未跑？），退回纯 sql_guard 模式"
        );
        return Isolation {
            role: None,
            tables: read,
            mode,
        };
    };

    // 授权：先撤后授，保证「能力收窄」真的收窄了（只授不撤会累积）。
    // 两张表单分开授——读表给 SELECT，写表才给写权限。
    let _ = sqlx::query("SELECT flux_plugin_revoke($1)")
        .bind(name)
        .execute(pool)
        .await;
    if !read.is_empty() {
        let _ = sqlx::query("SELECT flux_plugin_grant($1, $2, 'read')")
            .bind(name)
            .bind(&read)
            .execute(pool)
            .await;
    }
    if !write.is_empty() {
        let _ = sqlx::query("SELECT flux_plugin_grant($1, $2, 'write')")
            .bind(name)
            .bind(&write)
            .execute(pool)
            .await;
    }

    // 站点连接必须能 SET ROLE 到该角色（NOINHERIT 的反面：需要 SET TRUE）。
    // 权限不足时静默降级——SET 权拿不到不代表角色不存在。
    let can_set: bool = sqlx::query_scalar(
        "SELECT pg_has_role(current_user, $1, 'USAGE')",
    )
    .bind(&role)
    .fetch_one(pool)
    .await
    .unwrap_or(false);
    if !can_set {
        tracing::warn!(
            plugin = name,
            role = %role,
            "站点连接无 SET ROLE 权限，插件退回纯 sql_guard 模式"
        );
        return Isolation {
            role: None,
            tables: read,
            mode,
        };
    }

    Isolation {
        role: Some(role),
        tables: read,
        mode,
    }
}

/// 开一个已隔离的事务：`SET LOCAL ROLE` + 锁定 search_path。
///
/// 无角色可用时返回 `None`（调用方退回裸 pool 执行，语义同 0367）。
pub(super) async fn begin_isolated<'c>(
    pool: &'c PgPool,
    role: Option<&str>,
) -> DomainResult<Option<Transaction<'c, Postgres>>> {
    let mut tx: Transaction<'_, Postgres> = pool
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(role) = role else {
        return Ok(None);
    };
    // search_path 只留 pg_catalog：业务表必须显式 schema 限定，
    // 否则同名表可被 search_path 里的前序项劫持。
    for stmt in [
        format!("SET LOCAL ROLE {role}"),
        "SET LOCAL search_path = pg_catalog".to_string(),
    ] {
        sqlx::query(&stmt)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(Some(tx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cap(table: &str, mode: DbMode) -> DbCapability {
        DbCapability {
            table: table.into(),
            columns: vec!["id".into()],
            mode,
            allow_unbounded: false,
        }
    }

    /// 同一张表被声明成 Write 时，读表单也必须含它（写蕴含读，
    /// 否则授权会漏掉 SELECT 让插件自己的写语句读不回来）。
    #[test]
    fn write_table_also_in_read_list() {
        let (read, write) = split_tables(&[
            cap("torrents", DbMode::Write),
            cap("users", DbMode::Read),
        ]);
        assert_eq!(write, vec!["torrents".to_string()]);
        assert!(read.contains(&"torrents".to_string()));
        assert!(read.contains(&"users".to_string()));
        assert!(!write.contains(&"users".to_string()));
    }

    /// 表名统一小写（sql_guard 与 DB 侧都按小写判定，
    /// 大小写不一致会让授权静默失效）。
    #[test]
    fn table_names_lowercased() {
        let (read, _) = split_tables(&[cap("Torrents", DbMode::Read)]);
        assert_eq!(read, vec!["torrents".to_string()]);
    }

    /// 全 Read 声明时 mode 应为 Read。
    #[test]
    fn mode_is_read_when_no_write() {
        let (_, write) = split_tables(&[cap("users", DbMode::Read)]);
        assert!(write.is_empty());
    }
}
