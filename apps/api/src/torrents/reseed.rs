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
    let row: Option<(i32, String)> = sqlx::query_as(
        "SELECT seeders, name FROM torrents \
         WHERE id = $1 AND approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((seeders, name)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if seeders > 0 {
        return Err(DomainError::Validation(
            "该种子仍有人做种，无需补种".into(),
        ));
    }
    // 完成过下载的用户（含发布者）。P-378 三条收口：
    //  ① 排除被处置账号（`users.status >= 2` = 停用/墓碑/封禁）：旧版照发，
    //    现网聚合实测 176 封里有 100 封（57%）投给没人能登录的收件箱；
    //  ② 排除发起人自己（完成过该种的人会给自己发一封「我请求补种」）；
    //  ③ 发信与 `last_reseed` 落库并入一个事务：旧版逐条 autocommit，中途失败留下
    //    「半批人已收到、时间戳没更新」，重试时前半批**再收一遍**。
    // P-626：15 分钟限频的读取与写入必须原子。旧版事务外 SELECT 判限频、
    //  事务尾才 UPDATE：两个请求同时进来都读到旧时间戳 ⇒ 各发一整批。
    //  实测同一颗种 2ms 内两条 200/notified=1，收件人收到两封。改为在
    //  事务内用带谓词的 UPDATE 抢槽位；抢不到即被限频拒绝，事务回滚会
    //  释放这次抢占，下一个请求仍可正常发起。
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
    // P-629：零收件人时旧版照样回 200/notified=0 并占掉 15 分钟冷却，
    // 界面只会显示「已通知 0 位下载者，请耐心等待」。种主自己点补种、
    // 或唯一完成者已被处置时必然落到这一格：一封信都没发出就不该进
    // 限频窗口，直接回业务错，让界面说出真正的原因。
    if receivers.is_empty() {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation(
            "没有可通知的下载者（可能尚无人完成下载）".into(),
        ));
    }
    let claimed = sqlx::query(
        "UPDATE torrents SET last_reseed = now() WHERE id = $1 \
         AND (last_reseed IS NULL \
              OR last_reseed < now() - interval '15 minutes')",
    )
    .bind(torrent_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if claimed == 0 {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation(
            "15 分钟内已发起过补种请求，请稍候".into(),
        ));
    }
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
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(receivers.len())
}
