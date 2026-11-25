//! 账号/邀请 SQL 的居所。自 `repo/mod.rs` 拆出（300 行门禁）：
//! fixed_lottery 三模块注册后 mod 表净增，邀请三函数搬迁补偿，
//! 拆完 `mod.rs` 只留核心结构、用户查询与审计。

use crate::domain::new_passkey;
use crate::errors::{DomainError, DomainResult};
use crate::repo::Repo;

impl Repo {
    /// 尚无代码生效点：注册流程目前内联同语义 SQL（handler 事务内组合
    /// 其他校验）；后续如把注册迁入 repo 层则接线本方法，到时删本 allow。
    #[allow(dead_code)]
    pub async fn consume_invite(
        &self,
        code: &str,
        new_user_id: i64,
    ) -> DomainResult<Option<i64>> {
        let row = sqlx::query_scalar::<_, Option<i64>>(
            "UPDATE invites SET status = 1, used_by = $1 \
             WHERE code = $2 AND status = 0 AND expires_at > now() \
             RETURNING inviter_id",
        )
        .bind(new_user_id)
        .bind(code)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        match row {
            Some(inviter) => Ok(inviter),
            // 区分无效/已用（M01 错误码 2005/2006）
            None => {
                let used = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM invites \
                     WHERE code = $1 AND status = 1)",
                )
                .bind(code)
                .fetch_one(&self.db)
                .await
                .unwrap_or(false);
                if used {
                    Err(DomainError::InviteUsed)
                } else {
                    Err(DomainError::InviteInvalid)
                }
            }
        }
    }

    pub async fn create_user(
        &self,
        username: &str,
        email: &str,
        pass_hash: &str,
        invited_by: Option<i64>,
    ) -> DomainResult<i64> {
        let passkey = new_passkey();
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO users (username, email, pass_hash, passkey, \
             invited_by) VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(username)
        .bind(email)
        .bind(pass_hash)
        .bind(passkey)
        .bind(invited_by)
        .fetch_one(&self.db)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db) if db.is_unique_violation() => {
                DomainError::UsernameTaken
            }
            other => DomainError::Internal(other.into()),
        })?;
        Ok(id)
    }

    #[allow(dead_code)]
    pub async fn issue_invite(
        &self,
        inviter_id: i64,
        code: &str,
        expires_at: chrono::DateTime<chrono::Utc>,
    ) -> DomainResult<i64> {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO invites (inviter_id, code, expires_at) \
             VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(inviter_id)
        .bind(code)
        .bind(expires_at)
        .fetch_one(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(id)
    }
}
