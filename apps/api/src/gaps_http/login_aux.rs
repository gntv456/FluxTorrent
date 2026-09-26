//! 登录页辅助通道（0020）：封禁记录 + hash_token 助手。
//! 从 gaps_http.rs 按域拆出（mail.rs 的重置邮件也用 hash_token）。
//! （0216：/auth/confirm/resend 退役——注册无邮箱验证环节，端点恒返回「暂未启用」，
//!  「要么真驱动要么删」口径下删除。）

use actix_web::{get, web, Responder};
use redis::AsyncCommands;
use serde::Deserialize;
use sha3::{Digest, Sha3_256};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

pub(super) fn hash_token(t: &str) -> String {
    let mut h = Sha3_256::new();
    h.update(t.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ============ 登录页辅助通道（NexusPHP 口径：封禁记录 / 重发验证邮件） ============

/// 封禁记录（公开，防枚举只回状态与时间）：NexusPHP user-ban-log.php 对齐。
#[derive(Deserialize)]
struct BanLogQuery {
    username: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct BanLogRow {
    status: i16,
    changed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/auth/ban-log")]
pub async fn ban_log(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<BanLogQuery>,
) -> DomainResult<impl Responder> {
    // 限流：按用户名 5 次/分钟，防探测
    let mut c = state.redis.clone();
    let key = format!("rl:banlog:{}", q.username.trim().to_lowercase());
    let n: i64 = AsyncCommands::incr(&mut c, &key, 1).await.unwrap_or(0);
    if n == 1 {
        let _: () = AsyncCommands::expire(&mut c, &key, 60).await.unwrap_or(());
    }
    if n > 5 {
        return Err(DomainError::RateLimited);
    }
    // 最近一次对该用户的 status 变更（audit ref.id = 目标用户）
    let row = sqlx::query_as::<_, BanLogRow>(
        r#"
        SELECT u.status, (
            SELECT max(a.created_at) FROM audit_log a
            WHERE a.action = 'user.set_status'
              AND a.ref = jsonb_build_object('id', u.id)
        ) AS changed_at
        FROM users u WHERE lower(username::text) = lower($1) LIMIT 1
        "#,
    )
    .bind(q.username.trim())
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 无论命中与否同构响应（不泄露账号存在性）；无记录视为正常用户
    Ok(ok(serde_json::json!(row.unwrap_or(BanLogRow {
        status: 0,
        changed_at: None,
    }))))
}
