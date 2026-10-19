//! 求续种（NP takereseed.php 口径）：死种 → 给完成者发站内信，15 分钟限频。
//!
//! 从 `torrents/manage.rs` 拆出：manage.rs 顶到 300 行硬门（只许瘦不许胖），
//! 而求续种是一条自成链路的动作（校验 → 取收件人 → 发信 → 落时间戳），
//! 拆出来比把注释压扁更诚实。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 发起一次补种请求，返回真正收到信的人数。
pub async fn request_reseed(
    db: &PgPool,
    torrent_id: i64,
    requester: (i64, String),
) -> DomainResult<usize> {
    let row: Option<(i32, Option<chrono::DateTime<chrono::Utc>>, String)> =
        sqlx::query_as(
            "SELECT seeders, last_reseed, \
         name FROM torrents WHERE id = $1 AND approval_status = 1",
        )
        .bind(torrent_id)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((seeders, last_reseed, name)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if seeders > 0 {
        return Err(DomainError::Validation(
            "该种子仍有人做种，无需补种".into(),
        ));
    }
    if let Some(lr) = last_reseed {
        if chrono::Utc::now() - lr < chrono::Duration::seconds(900) {
            return Err(DomainError::Validation(
                "15 分钟内已发起过补种请求，请稍候".into(),
            ));
        }
    }
    // 完成过下载的用户（含发布者）。P-378 三条收口：
    //  ① 排除被处置账号（`users.status >= 2` = 停用/墓碑/封禁）：旧版照发，
    //    现网聚合实测 176 封里有 100 封（57%）投给没人能登录的收件箱；
    //  ② 排除发起人自己（完成过该种的人会给自己发一封「我请求补种」）；
    //  ③ 发信与 `last_reseed` 落库并入一个事务：旧版逐条 autocommit，中途失败留下
    //    「半批人已收到、时间戳没更新」，重试时前半批**再收一遍**。
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let receivers: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT s.user_id FROM snatches s \
         JOIN users u ON u.id = s.user_id \
         WHERE s.torrent_id = $1 AND s.completed_at IS NOT NULL \
           AND u.status < 2 AND u.id <> $2 \
         UNION \
         SELECT t.owner_id FROM torrents t \
         JOIN users u ON u.id = t.owner_id \
         WHERE t.id = $1 AND t.owner_id IS NOT NULL \
           AND u.status < 2 AND u.id <> $2",
    )
    .bind(torrent_id)
    .bind(requester.0)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let subject = "【补种请求】".to_string();
    let body = format!(
        "用户 {} 请求补种：{}（/torrent/{}）。如果你保留了文件，欢迎重新做种，谢谢！",
        requester.1, name, torrent_id
    );
    for uid in &receivers {
        sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, \
             subject, body, kind) VALUES ($1, $2, $3, $4, 'reseed_request')",
        )
        .bind(requester.0)
        .bind(uid)
        .bind(&subject)
        .bind(&body)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query("UPDATE torrents SET last_reseed = now() WHERE id = $1")
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(receivers.len())
}
