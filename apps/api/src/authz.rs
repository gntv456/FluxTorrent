//! 权限判定统一入口（RBAC）。
//!
//! 替代原先散落在 8 个文件里的 `auth.class_id < 90` 字面量阈值：
//! 过去只有 90/93/99 三档，改权限必须动代码并重新构建，也无法表达
//! 「发布员」这类**无高低之分的职能职务**。
//!
//! 现在权限来源取并集（满足其一即通过）：
//! ① **等级累进** —— `role_permissions` 中 `role_type='class'` 且 `class_id >= role_key`
//! ② **职务持有** —— `role_permissions` 中 `role_type='role'` 且用户持有该职务（`user_roles`，未过期）

use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;
use crate::state::AppState;
use actix_web::web;
use sqlx::PgPool;
use std::sync::Arc;

/// 权限键常量（与 0054 迁移的 permissions.key 一一对应，避免拼写漂移）
pub mod perm {
    // 发布
    pub const TORRENT_UPLOAD: &str = "torrent.upload";
    pub const TORRENT_APPROVAL_AUTO: &str = "torrent.approval.auto";
    /// 尚无代码生效点：付费种子改价端点未实装（价格仅发布时设置），接线时删除本 allow
    #[allow(dead_code)]
    pub const TORRENT_SET_PRICE: &str = "torrent.set_price";
    pub const TORRENT_VIEW_ANONYMOUS: &str = "torrent.view_anonymous";
    /// 尚无代码生效点（0062 迁移已标 implemented=false）：
    /// 「特殊分类」概念未定义，接线时删除本 allow
    #[allow(dead_code)]
    pub const TORRENT_UPLOAD_SPECIAL: &str = "torrent.upload_special";
    pub const TORRENT_SEE_BANNED: &str = "torrent.see_banned";
    /// 转载（repost）功能整体未实现（0062 迁移已标 implemented=false），接线时删除本 allow
    #[allow(dead_code)]
    pub const TORRENT_REPOST: &str = "torrent.repost";
    // 保种
    /// 由 worker 的 hr_enforce 批量 SQL 经 user_can() 函数消费（0056/0057），
    /// API 侧无常量引用点，保留以避免两边权限键拼写漂移
    #[allow(dead_code)]
    pub const HR_EXEMPT: &str = "hr.exempt";
    pub const SEED_STATS_VIEW: &str = "seed.stats.view";
    // 外联
    pub const INVITES_BONUS: &str = "invites.bonus";
    pub const ANNOUNCE_PUBLISH: &str = "announce.publish";
    // 内容
    pub const FAQ_MANAGE: &str = "faq.manage";
    pub const RULES_MANAGE: &str = "rules.manage";
    pub const NEWS_MANAGE: &str = "news.manage";
    pub const LINKS_MANAGE: &str = "links.manage";
    pub const FUN_MANAGE: &str = "fun.manage";
    pub const ADS_MANAGE: &str = "ads.manage";
    pub const CATEGORIES_MANAGE: &str = "categories.manage";
    pub const FORUMS_MANAGE: &str = "forums.manage";
    pub const POLLS_MANAGE: &str = "polls.manage";
    pub const OFFERS_PROMOTE: &str = "offers.promote";
    // 用户
    pub const USER_WARN: &str = "user.warn";
    pub const USER_STATUS: &str = "user.status";
    pub const USER_FLAGS: &str = "user.flags";
    pub const USER_ADJUST: &str = "user.adjust";
    pub const USER_CLASS: &str = "user.class";
    pub const USER_CREATE: &str = "user.create";
    pub const USER_RESETPASS: &str = "user.resetpass";
    pub const USER_DELETE_DISABLED: &str = "user.delete_disabled";
    pub const USER_AMOUNTBONUS: &str = "user.amountbonus";
    pub const USER_AMOUNTUPLOAD: &str = "user.amountupload";
    pub const APPEAL_HANDLE: &str = "appeal.handle";
    pub const IP_CHECK: &str = "ip.check";
    pub const UPLOADERS_VIEW: &str = "uploaders.view";
    pub const HR_PARDON: &str = "hr.pardon";
    pub const STAFF_PANEL: &str = "staff.panel";
    pub const STAFF_MESSAGE: &str = "staff.message";
    // 运营
    pub const STAFFMESS: &str = "staffmess";
    pub const MASSMAIL: &str = "massmail";
    pub const EMAILBAN_MANAGE: &str = "emailban.manage";
    pub const FREELEECH_VIEW: &str = "freeleech.view";
    pub const FREELEECH_MANAGE: &str = "freeleech.manage";
    pub const SETTINGS_VIEW: &str = "settings.view";
    pub const SETTINGS_MANAGE: &str = "settings.manage";
    pub const LOCATIONS_MANAGE: &str = "locations.manage";
    pub const SITEPACKS_MANAGE: &str = "sitepacks.manage";
    // 系统
    pub const AUDIT_VIEW: &str = "audit.view";
    pub const SYSLOG_VIEW: &str = "syslog.view";
    pub const DBSTATS_VIEW: &str = "dbstats.view";
    pub const STATS_VIEW: &str = "stats.view";
    pub const CLEANUP_RUN: &str = "cleanup.run";
    pub const CLEARCACHE: &str = "clearcache";
    pub const BANS_MANAGE: &str = "bans.manage";
    pub const TESTIP: &str = "testip";
    pub const MAXLOGIN_VIEW: &str = "maxlogin.view";
    pub const PLUGINS_MANAGE: &str = "plugins.manage";
    pub const AGENTS_VIEW: &str = "agents.view";
    pub const NOTCONNECTABLE_VIEW: &str = "notconnectable.view";
    // 第八轮 P3 管理套件（参考站后台逐页深挖落地）
    pub const TORRENT_MANAGE: &str = "torrent.manage";
    pub const HR_VIEW: &str = "hr.view";
    pub const INVITE_VIEW: &str = "invite.view";
    pub const ATTENDANCE_MANAGE: &str = "attendance.manage";
    pub const MEDAL_MANAGE: &str = "medal.manage";
    pub const PROP_MANAGE: &str = "prop.manage";
    pub const EXAM_MANAGE: &str = "exam.manage";
    pub const TASK_MANAGE: &str = "task.manage";
    pub const TRACKER_MANAGE: &str = "tracker.manage";
    pub const ROLES_MANAGE: &str = "roles.manage";
}

/// 判定用户是否拥有某权限。
///
/// 判定优先级：**用户级覆盖 > 角色权限**（`user_permissions` 有记录则以其为准，
/// 可额外授予或显式拒绝；无记录则继承角色判定）。
///
/// `role_key` 为 text 列，含 `'uploader'` 这类非数字值，故 class 分支用
/// `CASE` + 正则守卫，确保 `::integer` 只在确认是数字时求值
/// （PG 不保证 `AND` 的求值顺序，直接转换会抛 `invalid input syntax`）。
pub async fn user_can(
    db: &PgPool,
    class_id: i32,
    user_id: i64,
    perm: &str,
) -> bool {
    // 判定规则已下沉为 SQL 函数 user_can(uid, perm)（见 0056 迁移），
    // 使 worker 的批量 SQL 与 API 共用同一实现，避免两处规则漂移。
    // class_id 参数保留仅为兼容既有调用方（函数内部自行读 users.class_id）。
    let _ = class_id;
    sqlx::query_scalar("SELECT user_can($1, $2)")
        .bind(user_id)
        .bind(perm)
        .fetch_one(db)
        .await
        .unwrap_or(false)
}

/// 便捷判定：从已鉴权用户出发（多数端点用这个）
pub async fn can(
    state: &web::Data<Arc<AppState>>,
    auth: &AuthUser,
    perm: &str,
) -> bool {
    user_can(&state.repo.db, auth.class_id, auth.id, perm).await
}

/// 端点守卫（任一通过）：用于「多角色可做同一件事」的场景，
/// 例如新闻发布允许 news.manage（内容管理）或 announce.publish（外联员代表站点发声）。
pub async fn require_any_perm(
    state: &web::Data<Arc<AppState>>,
    auth: &AuthUser,
    perms: &[&str],
) -> DomainResult<()> {
    for p in perms {
        if can(state, auth, p).await {
            return Ok(());
        }
    }
    Err(DomainError::Forbidden)
}

/// 端点守卫：无权限直接返回 Forbidden
pub async fn require_perm(
    state: &web::Data<Arc<AppState>>,
    auth: &AuthUser,
    perm: &str,
) -> DomainResult<()> {
    if can(state, auth, perm).await {
        Ok(())
    } else {
        Err(DomainError::Forbidden)
    }
}

/// 批量判定：一次性取回用户拥有的全部权限键
/// （前端渲染功能入口时用，避免 N 次 round-trip）
pub async fn user_perm_keys(
    db: &PgPool,
    class_id: i32,
    user_id: i64,
) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT DISTINCT key FROM (
            SELECT rp.permission_key AS key FROM role_permissions rp
            WHERE rp.granted
              AND (
                (rp.role_type = 'class'
                 AND CASE WHEN rp.role_key ~ '^[0-9]+$'
                          THEN $1 >= rp.role_key::integer
                          ELSE FALSE END)
                OR
                (rp.role_type = 'role'
                 AND EXISTS (
                     SELECT 1 FROM user_roles ur
                     WHERE ur.user_id = $2
                       AND ur.role_key = rp.role_key
                       AND (ur.expires_at IS NULL OR ur.expires_at > now())
                 ))
              )
            UNION
            SELECT up.permission_key FROM user_permissions up
            WHERE up.user_id = $2 AND up.granted
         ) t
         WHERE key NOT IN (
             SELECT permission_key FROM user_permissions
             WHERE user_id = $2 AND NOT granted
         )
         ORDER BY 1",
    )
    .bind(class_id)
    .bind(user_id)
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

/// 列出用户当前持有的职务（未过期的）
pub async fn user_role_keys(db: &PgPool, user_id: i64) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT role_key FROM user_roles
         WHERE user_id = $1 AND (expires_at IS NULL OR expires_at > now())
         ORDER BY granted_at",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

/// 授予职务（幂等；已存在则刷新有效期与授予人）
pub async fn grant_role(
    db: &PgPool,
    user_id: i64,
    role_key: &str,
    actor_id: i64,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_roles (user_id, role_key, granted_by, expires_at)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (user_id, role_key) DO UPDATE SET
            granted_by = EXCLUDED.granted_by,
            granted_at = now(),
            expires_at = EXCLUDED.expires_at",
    )
    .bind(user_id)
    .bind(role_key)
    .bind(actor_id)
    .bind(expires_at)
    .execute(db)
    .await?;
    Ok(())
}

/// 撤销职务
pub async fn revoke_role(
    db: &PgPool,
    user_id: i64,
    role_key: &str,
) -> Result<u64, sqlx::Error> {
    let r = sqlx::query(
        "DELETE FROM user_roles WHERE user_id = $1 AND role_key = $2",
    )
    .bind(user_id)
    .bind(role_key)
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}

/// 读取用户的权限级覆盖记录（permission_key → granted）
pub async fn user_permission_overrides(
    db: &PgPool,
    user_id: i64,
) -> Vec<(String, bool)> {
    sqlx::query_as(
        "SELECT permission_key, granted FROM user_permissions
         WHERE user_id = $1 ORDER BY permission_key",
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .unwrap_or_default()
}

/// 设置或清除用户级权限覆盖。
/// `granted = Some(true)` 额外授予 / `Some(false)` 显式拒绝 / `None` 清除覆盖（回归角色判定）
pub async fn set_user_permission(
    db: &PgPool,
    user_id: i64,
    permission_key: &str,
    granted: Option<bool>,
    actor_id: i64,
    note: Option<&str>,
) -> Result<(), sqlx::Error> {
    match granted {
        None => {
            sqlx::query("DELETE FROM user_permissions WHERE user_id = $1 AND permission_key = $2")
                .bind(user_id)
                .bind(permission_key)
                .execute(db)
                .await?;
        }
        Some(g) => {
            sqlx::query(
                "INSERT INTO user_permissions (user_id, permission_key, granted, granted_by, note)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (user_id, permission_key) DO UPDATE SET
                    granted = EXCLUDED.granted,
                    granted_by = EXCLUDED.granted_by,
                    granted_at = now(),
                    note = EXCLUDED.note",
            )
            .bind(user_id)
            .bind(permission_key)
            .bind(g)
            .bind(actor_id)
            .bind(note)
            .execute(db)
            .await?;
        }
    }
    Ok(())
}
