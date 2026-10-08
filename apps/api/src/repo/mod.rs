//! 仓储层：sqlx 参数化查询（§8.3.1：一律参数化，编译期检查由 CI sqlx prepare 承担）。
//! 模块化（2026-09-20 repo 分层做实）：单文件 → 目录；auth.rs 收编注册事务，
//! 后续域（torrents/economy…）按同模式渐进。

use sqlx::PgPool;

use crate::domain::{new_passkey, UserAccount};
use crate::errors::{DomainError, DomainResult};

pub struct Repo {
    pub db: PgPool,
    /// 只读池（0230 G31-D2）：DATABASE_REPLICA_URL 未配置时与主池同一句柄
    /// （Arc 克隆，零开销零行为变化）；配置后热点读走副本。
    /// ⚠️ 写后立读路径（发布跳转/支付状态/刚写完的实体）必须用 db，
    /// 复制延迟会让用户看到旧数据——钉主库清单见配方文档 §8。
    pub read_db: PgPool,
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
    archived: bool,
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
            archived: r.archived,
        }
    }
}

impl Repo {
    /// 同步构造（测试用）：read_db = 主池克隆。
    /// 读并回落（审查 P1-10）：副本运行期故障时热点读降级主库而非 500。
    /// 用法：repo.read_fallback(|db| query...fetch_one(db)).await
    pub async fn read_fallback<T, E, F, Fut>(&self, f: F) -> Result<T, E>
    where
        F: Fn(&PgPool) -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        match f(&self.read_db).await {
            Ok(v) => Ok(v),
            Err(e) => {
                // 只对连接类错误回落（业务错误照常返回）——E 无类型细节，
                // 简化：全部回落（业务错误重试一次代价可接受且幂等）
                tracing::warn!("read_db 查询失败，回落主库");
                f(&self.db).await.or(Err(e))
            }
        }
    }

    #[allow(dead_code)]
    pub fn new(db: PgPool) -> Self {
        Self {
            read_db: db.clone(),
            db,
        }
    }

    /// 副本池构造：None / 连接失败时退化为主池克隆（启动不因副本故障失败）。
    pub async fn with_replica(db: PgPool, replica_url: Option<&str>) -> Self {
        let read_db = match replica_url {
            Some(url) if !url.is_empty() => {
                match sqlx::postgres::PgPoolOptions::new()
                    .max_connections(8)
                    .acquire_timeout(std::time::Duration::from_secs(3))
                    .connect(url)
                    .await
                {
                    Ok(p) => {
                        tracing::info!("读写分离已启用：热点读走只读副本");
                        p
                    }
                    Err(e) => {
                        tracing::warn!(
                            ?e,
                            "只读副本连接失败，read_db 退化为主池"
                        );
                        db.clone()
                    }
                }
            }
            _ => db.clone(),
        };
        Self { db, read_db }
    }

    pub async fn find_user_by_name(
        &self,
        username: &str,
    ) -> DomainResult<Option<UserAccount>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, pass_hash, passkey, class_id, must_reset_password, dormant_at, archived \
             FROM users WHERE username = $1 AND status < 2",
        )
        .bind(username)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(row.map(Into::into))
    }

    pub async fn find_user_by_id(
        &self,
        id: i64,
    ) -> DomainResult<Option<UserAccount>> {
        let row = sqlx::query_as::<_, UserRow>(
            "SELECT id, username, pass_hash, passkey, class_id, must_reset_password, dormant_at, archived \
             FROM users WHERE id = $1 AND status < 2",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        Ok(row.map(Into::into))
    }

    /// 邀请码消费：原子 UPDATE 保证一码一用（M01 关键规则）。
    /// 尚无代码生效点：注册流程目前内联同语义 SQL（handler 事务内组合其他校验），
    /// 后续如把注册迁入 repo 层则接线本方法，到时删除本 allow
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
            None => {
                // 区分无效/已用（M01 错误码 2005/2006）
                let used: bool = sqlx::query_scalar(
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

    pub async fn audit(
        &self,
        actor_id: Option<i64>,
        action: &str,
        ref_id: Option<i64>,
    ) {
        self.audit_detail(actor_id, action, ref_id, None, None)
            .await;
    }

    /// 带理由与附加现场的审计（0285）。
    /// 旧版 `ref` 只存 `{"id": 目标}`，叠加读模型根本不选 ref，
    /// 于是「谁把这名用户的余额改成多少、为什么」无法回答——版主交接与纠纷仲裁
    /// 全靠这三问。`reason` 按字符边界截断（多字节 UA/文案曾按字节切片 panic，ZT81 同因）。
    pub async fn audit_detail(
        &self,
        actor_id: Option<i64>,
        action: &str,
        ref_id: Option<i64>,
        reason: Option<&str>,
        extra: Option<serde_json::Value>,
    ) {
        let mut v = serde_json::Map::new();
        if let Some(id) = ref_id {
            v.insert("id".to_string(), serde_json::json!(id));
        }
        if let Some(r) = reason.map(str::trim).filter(|s| !s.is_empty()) {
            v.insert(
                "reason".to_string(),
                serde_json::json!(r.chars().take(200).collect::<String>()),
            );
        }
        if let Some(serde_json::Value::Object(m)) = extra {
            for (k, val) in m {
                v.insert(k, val);
            }
        }
        let payload = if v.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(v).to_string())
        };
        // 审计日志失败不阻塞业务，但必须记录（§5.7）
        let audit = sqlx::query(
            "INSERT INTO audit_log (id, actor_id, action, ref) \
             VALUES (nextval('audit_log_id_seq'), $1, $2, $3::jsonb)",
        )
        .bind(actor_id)
        .bind(action)
        .bind(payload)
        .execute(&self.db)
        .await;
        if let Err(e) = &audit {
            tracing::error!(?e, "审计日志写入失败（§5.7 要求必记）");
        }
    }
}

pub mod auth;
pub mod passkey;
