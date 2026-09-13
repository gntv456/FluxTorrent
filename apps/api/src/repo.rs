//! 仓储层：sqlx 参数化查询（§8.3.1：一律参数化，编译期检查由 CI sqlx prepare 承担）。

use sqlx::PgPool;

use crate::domain::{new_passkey, UserAccount};
use crate::errors::{DomainError, DomainResult};

pub struct Repo {
    pub db: PgPool,
}

#[derive(sqlx::FromRow)]
struct UserRow {
    id: i64,
    username: String,
    pass_hash: String,
    passkey: String,
    class_id: i32,
    must_reset_password: bool,
    dormant_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<UserRow> for UserAccount {
    fn from(r: UserRow) -> Self {
        UserAccount {
            id: r.id,
            username: r.username,
            pass_hash: r.pass_hash,
            passkey: r.passkey,
            class_id: r.class_id,
            must_reset_password: r.must_reset_password,
            dormant_at: r.dormant_at,
        }
    }
}

impl Repo {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn find_user_by_name(&self, username: &str) -> DomainResult<Option<UserAccount>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, pass_hash, passkey, class_id, must_reset_password, dormant_at \
             FROM users WHERE username = $1 AND status < 2",
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(row.map(Into::into))
    }

    pub async fn find_user_by_id(&self, id: i64) -> DomainResult<Option<UserAccount>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, pass_hash, passkey, class_id, must_reset_password, dormant_at \
             FROM users WHERE id = $1 AND status < 2",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(row.map(Into::into))
    }

    /// 邀请码消费：原子 UPDATE 保证一码一用（M01 关键规则）
    pub async fn consume_invite(&self, code: &str, new_user_id: i64) -> DomainResult<Option<i64>> {
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
            None => {
                // 区分无效/已用（M01 错误码 2005/2006）
                let used: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM invites WHERE code = $1 AND status = 1)",
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
            "INSERT INTO users (username, email, pass_hash, passkey, invited_by) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(username)
        .bind(email)
        .bind(pass_hash)
        .bind(passkey)
        .bind(invited_by)
        .fetch_one(&self.db)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(db) if db.is_unique_violation() => DomainError::UsernameTaken,
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
            "INSERT INTO invites (inviter_id, code, expires_at) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(inviter_id)
        .bind(code)
        .bind(expires_at)
        .fetch_one(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(id)
    }

    pub async fn update_passkey(&self, user_id: i64) -> DomainResult<String> {
        let pk = new_passkey();
        sqlx::query("UPDATE users SET passkey = $2 WHERE id = $1")
            .bind(user_id)
            .bind(&pk)
            .execute(&self.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(pk)
    }

    pub async fn audit(&self, actor_id: Option<i64>, action: &str, ref_id: Option<i64>) {
        // 审计日志失败不阻塞业务，但必须记录（§5.7）
        let audit = sqlx::query(
            "INSERT INTO audit_log (id, actor_id, action, ref) VALUES (nextval('audit_log_id_seq'), $1, $2, $3::jsonb)",
        )
        .bind(actor_id)
        .bind(action)
        .bind(ref_id.map(|i| serde_json::json!({"id": i}).to_string()))
        .execute(&self.db)
        .await;
        if let Err(e) = &audit {
            tracing::error!(?e, "审计日志写入失败（§5.7 要求必记）");
        }
    }
}
