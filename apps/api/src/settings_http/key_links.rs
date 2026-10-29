//! 设置键联锁（从 groups.rs / site_put.rs 两处收敛，P-680 语义单源）。
//!
//! register_open（NP yesno 口径）与 registration_mode（三态枚举）是同一
//! 件事的两张脸：开关打开而模式还是 invite_only 时，注册页照样强制
//! 邀请码——「设置说不用邀请码、注册页还是要」。保存任一侧都把另一侧
//! 拉齐；只镜像 开关→模式 方向（三态枚举比开关细，反向会把
//! email_verify 压扁成 invite_only）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 非事务版（site_put 单项口直连池）。
pub(crate) async fn sync_registration_mode(
    state: &AppState,
    register_open_value: &str,
) -> DomainResult<()> {
    let mut conn = state
        .repo
        .db
        .acquire()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sync_registration_mode_tx(&mut conn, register_open_value).await
}

/// 事务内版本（groups 批量保存在同一事务里跑，联动必须同生共死）。
pub(crate) async fn sync_registration_mode_tx(
    tx: &mut sqlx::PgConnection,
    register_open_value: &str,
) -> DomainResult<()> {
    let target = if register_open_value == "yes" {
        "open"
    } else {
        "invite_only"
    };
    sqlx::query(
        "UPDATE site_settings SET value = $1, updated_at = now() \
         WHERE name = 'registration_mode' \
         AND value IS DISTINCT FROM $1",
    )
    .bind(target)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// site_name→SITENAME 单向镜像（0214 P2-4.6）：site_name 是权威键，SITENAME
/// （NP 旧口径，RSS 频道名在读）随写同步。只镜像正向——直接改 SITENAME
/// 不回写（反向同步会在两键同改时产生写冲突）。
pub(crate) async fn mirror_sitename_tx(
    tx: &mut sqlx::PgConnection,
    site_name: &str,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE site_settings SET value = $2, updated_at = now() \
         WHERE name = 'SITENAME' AND value IS DISTINCT FROM $2",
    )
    .bind("SITENAME")
    .bind(site_name)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}
