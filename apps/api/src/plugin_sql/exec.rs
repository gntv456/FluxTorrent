//! 隔离执行面（plugin_sql/mod.rs 的执行三件套）。
//!
//! 三件套同构：有角色走「事务 + SET LOCAL ROLE + 锁 search_path」，
//! 没角色走裸 pool（= 0367 的既有强度）。降级分支刻意保留，
//! 保证自部署缺 CREATEROLE 时行为与迁移 0378 之前完全一致。

use sqlx::PgPool;
use sqlx::postgres::{PgArguments, PgQueryResult, PgRow};
use sqlx::Postgres;

use crate::errors::{DomainError, DomainResult};
use crate::plugin_ctx::Bind;

use super::{begin_isolated, Isolation};

trait BindVal: Sized {
    fn bind_val(self, b: &crate::plugin_ctx::Bind) -> Self;
}

impl<'q> BindVal for sqlx::query::Query<'q, Postgres, PgArguments> {
    fn bind_val(self, b: &crate::plugin_ctx::Bind) -> Self {
        use crate::plugin_ctx::Bind as B;
        match b {
            B::I64(v) => self.bind(*v),
            B::OptI64(v) => self.bind(*v),
            B::Str(s) => self.bind(s.clone()),
            B::OptStr(s) => self.bind(s.clone()),
            B::Bool(v) => self.bind(*v),
        }
    }
}

impl<'q> BindVal
    for sqlx::query::QueryScalar<'q, Postgres, i64, PgArguments>
{
    fn bind_val(self, b: &crate::plugin_ctx::Bind) -> Self {
        use crate::plugin_ctx::Bind as B;
        match b {
            B::I64(v) => self.bind(*v),
            B::OptI64(v) => self.bind(*v),
            B::Str(s) => self.bind(s.clone()),
            B::OptStr(s) => self.bind(s.clone()),
            B::Bool(v) => self.bind(*v),
        }
    }
}

/// 隔离执行：写/读语句（无行返回）。sql_guard 已由调用方跑过。
pub async fn exec_isolated(
    pool: &PgPool,
    iso: &Isolation,
    sql: &str,
    binds: &[crate::plugin_ctx::Bind],
) -> DomainResult<PgQueryResult> {
    match begin_isolated(pool, iso.role()).await? {
        Some(mut tx) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind_val(b);
            }
            let r = q.execute(&mut *tx).await;
            let _ = tx.commit().await;
            r.map_err(|e| DomainError::Internal(e.into()))
        }
        None => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind_val(b);
            }
            q.execute(pool)
                .await
                .map_err(|e| DomainError::Internal(e.into()))
        }
    }
}

/// 隔离执行：多行查询。
pub async fn fetch_all_isolated(
    pool: &PgPool,
    iso: &Isolation,
    sql: &str,
    binds: &[crate::plugin_ctx::Bind],
) -> DomainResult<Vec<PgRow>> {
    match begin_isolated(pool, iso.role()).await? {
        Some(mut tx) => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind_val(b);
            }
            let r = q.fetch_all(&mut *tx).await;
            let _ = tx.commit().await;
            r.map_err(|e| DomainError::Internal(e.into()))
        }
        None => {
            let mut q = sqlx::query(sql);
            for b in binds {
                q = q.bind_val(b);
            }
            q.fetch_all(pool)
                .await
                .map_err(|e| DomainError::Internal(e.into()))
        }
    }
}

/// 隔离执行：单值 i64。
pub async fn fetch_scalar_i64_isolated(
    pool: &PgPool,
    iso: &Isolation,
    sql: &str,
    binds: &[crate::plugin_ctx::Bind],
) -> DomainResult<Option<i64>> {
    match begin_isolated(pool, iso.role()).await? {
        Some(mut tx) => {
            let mut q = sqlx::query_scalar::<_, i64>(sql);
            for b in binds {
                q = q.bind_val(b);
            }
            let r = q.fetch_optional(&mut *tx).await;
            let _ = tx.commit().await;
            r.map_err(|e| DomainError::Internal(e.into()))
        }
        None => {
            let mut q = sqlx::query_scalar::<_, i64>(sql);
            for b in binds {
                q = q.bind_val(b);
            }
            q.fetch_optional(pool)
                .await
                .map_err(|e| DomainError::Internal(e.into()))
        }
    }
}
