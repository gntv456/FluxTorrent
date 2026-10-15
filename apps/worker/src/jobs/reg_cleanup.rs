//! 注册治理清理（0370 M4）。
//! reg_ip_signups 只存 sha3-256(ip) 摘要（隐私合规：库中无明文 IP），
//! 保留期由 site_settings.reg_ip_retention_days 控制（默认 30 天）；
//! 到期摘要在同 IP 限额窗口（reg_ip_window_hours，默认 24h）之外已无用途。
//!
//! 每日一把 DELETE（走 idx_reg_ip_signups_created），幂等。

use sqlx::PgPool;

pub(crate) async fn reg_cleanup(db: &PgPool) -> anyhow::Result<u64> {
    let days: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'reg_ip_retention_days'), 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30)
    .clamp(1, 3650);
    let n = sqlx::query(
        "DELETE FROM reg_ip_signups \
         WHERE created_at < now() - ($1::bigint * interval '1 day')",
    )
    .bind(days)
    .execute(db)
    .await?
    .rows_affected();
    if n > 0 {
        tracing::info!(n, days, "reg_cleanup: 过期注册摘要已清理");
    }
    Ok(n)
}
