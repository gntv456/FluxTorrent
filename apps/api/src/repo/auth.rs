//! 仓储模块目录（repo 分层做实第一步，2026-09-20）：
//! repo.rs 保留鉴权/用户基础方法；多语句业务事务逐步收编为各域仓储方法。
//! 收编顺序按审查建议：register 事务（本文件）→ 其余按域渐进。

use sqlx::PgPool;

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
        let user_id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, email, pass_hash, \
             passkey, uploaded) VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(&new_user.username)
        .bind(&new_user.email)
        .bind(pass_hash)
        .bind(&passkey)
        .bind(initial_uploaded)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| match e {
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
                 used_by = $1 WHERE code = $2 AND status = 0 AND expires_at > now() RETURNING inviter_id",
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
