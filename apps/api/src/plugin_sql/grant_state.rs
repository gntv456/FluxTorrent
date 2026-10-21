//! 插件角色授权的台账比对与临界区（自 mod.rs 拆出，守 300 行门禁）。
//!
//! 背景（2026-10-21 缺陷修复）：`ensure_isolation` 旧实现每次分发都
//! 「revoke 全撤 → 再 grant」。发种是双重分发（upload.rs 内联
//! `dispatch_upload` + 领域事件消费端 `dispatch_upload_pool`），两个
//! ensure_isolation 并发跑时，一个的 `REVOKE ALL PRIVILEGES` 会把插件
//! 角色授权清零，另一个正在执行的插件 SQL 当场
//! 「permission denied for table」（现网实证：官种置顶插件每次发种报错）。
//! 修法：台账（plugin_sql_roles）一致就走快路不撤不授；只有声明漂移时
//! 才进临界区撤授，且授权随事务提交原子可见。

use sqlx::PgPool;

/// 集合相等（顺序无关）——台账 tables 与声明清单比对用。
pub(super) fn same_set(a: &[String], b: &[String]) -> bool {
    let sa: std::collections::BTreeSet<&str> =
        a.iter().map(String::as_str).collect();
    let sb: std::collections::BTreeSet<&str> =
        b.iter().map(String::as_str).collect();
    sa == sb
}

/// 插件角色撤授临界区的咨询锁键（定值 'flux'；仅声明漂移的慢路持锁）。
pub(super) const PLUGIN_GRANT_LOCK: i64 = 0x666C_7578;

/// 台账终态：grant 先写读行、再被写行 upsert 覆盖，最终行反映写侧。
pub(super) fn ledger_expectation(
    read: &[String],
    write: &[String],
) -> (Vec<String>, &'static str) {
    if write.is_empty() {
        (read.to_vec(), "read")
    } else {
        (write.to_vec(), "write")
    }
}

/// 查台账 + 角色在册状态（快路与慢路双检共用）。
/// 泛型执行器：快路走 pool，慢路双检必须走事务内（`&mut *tx`），
/// 否则双检读的是锁外快照，等于没检。
pub(super) async fn ledger_state<'e, E>(
    exec: E,
    name: &str,
    role: &str,
) -> Option<(Vec<String>, String, bool)>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as(
        "SELECT l.tables, l.mode::text, \
         EXISTS(SELECT 1 FROM pg_roles WHERE rolname = $2) \
         FROM plugin_sql_roles l WHERE l.plugin = $1",
    )
    .bind(name)
    .bind(role)
    .fetch_optional(exec)
    .await
    .ok()
    .flatten()
}

/// 事务内撤授临界区：咨询锁 + 先撤后授，提交后授权对其它连接原子可见。
///
/// 返回 `false` = 降级（开事务/拿锁/提交任一步失败），调用方应退回
/// 纯 sql_guard 模式。
pub(super) async fn grant_in_critical_section(
    pool: &PgPool,
    name: &str,
    read: &[String],
    write: &[String],
) -> bool {
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::warn!(plugin = name, ?e, "插件授权事务开启失败");
            return false;
        }
    };
    if let Err(e) = sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(PLUGIN_GRANT_LOCK)
        .execute(&mut *tx)
        .await
    {
        tracing::warn!(plugin = name, ?e, "插件授权锁获取失败");
        return false;
    }
    // 先撤后授，保证「能力收窄」真的收窄了（只授不撤会累积）。
    // 两张表单分开授——读表给 SELECT，写表才给写权限。
    let _ = sqlx::query("SELECT flux_plugin_revoke($1)")
        .bind(name)
        .execute(&mut *tx)
        .await;
    if !read.is_empty() {
        let _ = sqlx::query("SELECT flux_plugin_grant($1, $2, 'read')")
            .bind(name)
            .bind(read)
            .execute(&mut *tx)
            .await;
    }
    if !write.is_empty() {
        let _ = sqlx::query("SELECT flux_plugin_grant($1, $2, 'write')")
            .bind(name)
            .bind(write)
            .execute(&mut *tx)
            .await;
    }
    if let Err(e) = tx.commit().await {
        tracing::warn!(plugin = name, ?e, "插件授权事务提交失败");
        return false;
    }
    true
}
