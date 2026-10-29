//! 仓储模块目录（repo 分层做实第一步，2026-09-20）：
//! repo.rs 保留鉴权/用户基础方法；多语句业务事务逐步收编为各域仓储方法。
//! 收编顺序按审查建议：register 事务（本文件）→ 其余按域渐进。

use sqlx::PgPool;

use super::Repo;
use crate::domain::NewUser;
use crate::errors::{DomainError, DomainResult};

pub struct AuthRepo {
    pub db: PgPool,
}

impl AuthRepo {
    /// 注册事务（自 auth_http::register 收编）：建用户 + 消费邀请码 + 绑邀请人
    /// 单事务（审计修复口径原样保留——中间崩溃不留未绑邀请人的账号）。
    /// 返回 (user_id, inviter)；邀请码无效时整体回滚并返回 InviteUsed/InviteInvalid。
    pub async fn register_user(
        &self,
        new_user: &NewUser,
        pass_hash: &str,
        invite_code: &str,
        invite_only: bool,
    ) -> DomainResult<(i64, Option<i64>)> {
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        // 用户总量闸（0370 M4）：maxusers（复用既有键，0 = 不限）。
        // 必须**在本事务内**校验并持 advisory 锁——NexusPHP 的
        // check-then-insert 竞态会让并发注册各自通过检查、一起建号突破上限。
        // xact 级锁在 commit/rollback 时自动释放，只串行化「计数→插入」段。
        const REG_CAP_LOCK: i64 = 0x7265_675f_6361_7001;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(REG_CAP_LOCK)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let max_users: i64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value::bigint FROM site_settings \
             WHERE name = 'maxusers'), 0)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0);
        if max_users > 0 {
            let used: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM users WHERE status < 2",
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
            if used >= max_users {
                return Err(DomainError::Validation(
                    "本站注册人数已达上限，请联系管理组".into(),
                ));
            }
        }
        // passkey 列是 CHAR(32)：gen_random_bytes(20)→hex 是 40 位必溢出。
        // 与 update_passkey 的 new_passkey()（32 位小写字母数字）对齐。
        let passkey = crate::domain::new_passkey();
        // 初始上传量（C4 缓冲宽进制）：initial_upload_gb > 0 时新用户出生即带
        // 缓冲（UNIT3D default_upload 口径）；键缺失/0 = 维持旧行为。
        // value 是 TEXT：坏值（手改库写非数字）按 0 处理，不阻断注册。
        let initial_gb: f64 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value FROM site_settings \
             WHERE name = 'initial_upload_gb')::float8, 0)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(0.0);
        let initial_uploaded: i64 =
            (initial_gb.max(0.0) * 1024.0 * 1024.0 * 1024.0) as i64;
        // 初始等级（深测四轮 2026-10-03）：此前 INSERT 不写 class_id → 落 DB
        // 缺省 0（「种子」级，role_permissions 零授权），新用户注册后连发种都
        // 403，要等最长 5 分钟 worker class_auto_adjust 才提到 1（新芽）。
        // 站长可用 initial_class 设定改初始档（0..89 有效档；<90 防误配把
        // 注册直通 staff）。缺省 1 = 与权限种子（torrent.upload 授 class≥1）对齐。
        let initial_class: i32 = sqlx::query_scalar(
            "SELECT COALESCE((SELECT value FROM site_settings \
             WHERE name = 'initial_class')::int, 1)",
        )
        .fetch_one(&mut *tx)
        .await
        .unwrap_or(1)
        .clamp(0, 89);
        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, email, pass_hash, \
             passkey, uploaded, class_id) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(&new_user.username)
        .bind(&new_user.email)
        .bind(pass_hash)
        .bind(&passkey)
        .bind(initial_uploaded)
        .bind(initial_class)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
            // P-688：unique 冲突按约束名分流——email 撞 users_email_key
            // 时旧写法也报「用户名已被占用」，用户名明明没占，指着错误的
            // 字字让用户改错东西（现网：gntv 用已注册邮箱，被引导去换
            // 用户名，怎么换都失败）。
            sqlx::Error::Database(db)
                if db.is_unique_violation()
                    && db.constraint().is_some_and(|c| {
                        c.contains("email")
                    }) =>
            {
                DomainError::Validation(
                    "该邮箱已被注册（如忘记密码可在登录页找回，或换一个邮箱）"
                        .into(),
                )
            }
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                DomainError::UsernameTaken
            }
            other => DomainError::Internal(other.into()),
        })?;
        let inviter: Option<i64> = if invite_only {
            // 邮箱定向邀请（评审 2026-09-27）：邀请码经 /invites/email 发送时
            // invites.email 记录了目标邮箱——消费时校验注册邮箱一致，防止
            // 码被转发后任何邮箱都能用（NP 邮件邀请口径）。无绑定（NULL，
            // 复制口头发送）不受影响。
            let bound_email: Option<Option<String>> = sqlx::query_scalar(
                "SELECT email FROM invites \
                 WHERE code = $1 AND status = 0 AND expires_at > now()",
            )
            .bind(invite_code)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if let Some(expected) =
                bound_email.flatten().as_deref().map(str::trim)
            {
                if !expected.is_empty()
                    && !expected.eq_ignore_ascii_case(new_user.email.trim())
                {
                    return Err(DomainError::Validation(
                        "该邀请码是定向邀请，注册邮箱须与收到邀请的邮箱一致"
                            .into(),
                    ));
                }
            }
            let inv = sqlx::query_scalar(
                                "UPDATE invites SET status = 1, \
                 used_by = $1 WHERE code = $2 AND status = 0 AND expires_at > now()                  AND (SELECT status FROM users WHERE id = invites.inviter_id) < 2                  RETURNING inviter_id",
            )
            .bind(user_id)
            .bind(invite_code)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(inv) = inv else {
                // 邀请码无效：先区分已用/无效再回滚（区分查询走池连接，事务随后 drop 回滚）
                let used: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM invites \
                     WHERE code = $1 AND status = 1)",
                )
                .bind(invite_code)
                .fetch_one(&self.db)
                .await
                .unwrap_or(false);
                return Err(if used {
                    DomainError::InviteUsed
                } else {
                    DomainError::InviteInvalid
                });
            };
            Some(inv)
        } else {
            None
        };
        if let Some(inviter) = inviter {
            sqlx::query("UPDATE users SET invited_by = $2 WHERE id = $1")
                .bind(user_id)
                .bind(inviter)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok((user_id, inviter))
    }
}

/// 新手清单的**可核验**事实：每一项都来自真实痕迹，不接收点击自报。
#[derive(sqlx::FromRow)]
pub struct OnboardingFacts {
    pub email_verified: bool,
    pub email_verify_mode: bool,
    pub totp_enabled: bool,
    pub passkey: bool,
    pub rss_token: bool,
    pub profile: bool,
    pub first_download: bool,
    pub first_seed: bool,
}

pub async fn onboarding_facts(
    repo: &Repo,
    uid: i64,
) -> Result<Option<OnboardingFacts>, sqlx::Error> {
    sqlx::query_as::<_, OnboardingFacts>(
        "SELECT \
           (u.email_verified_at IS NOT NULL) AS email_verified, \
           COALESCE((SELECT value FROM site_settings \
                     WHERE name = 'registration_mode'), \
                    'invite_only') = 'email_verify' \
             AS email_verify_mode, \
           u.totp_enabled, \
           EXISTS(SELECT 1 FROM user_passkeys p \
                  WHERE p.user_id = u.id) AS passkey, \
           EXISTS(SELECT 1 FROM rss_tokens t \
                  WHERE t.user_id = u.id \
                    AND t.revoked_at IS NULL) AS rss_token, \
           (COALESCE(u.avatar_url, '') <> '' \
            OR COALESCE(u.info, '') <> '') AS profile, \
           EXISTS(SELECT 1 FROM snatches s \
                  WHERE s.user_id = u.id \
                    AND s.downloaded > 0 \
                    AND s.completed_at IS NOT NULL) AS first_download, \
           EXISTS(SELECT 1 FROM snatches s \
                  WHERE s.user_id = u.id \
                    AND (s.seeded_seconds > 0 \
                         OR s.uploaded > 0)) AS first_seed \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_optional(&repo.db)
    .await
}
