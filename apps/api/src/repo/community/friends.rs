//! 好友操作（POST /friends/action）的 SQL 唯一居所。
//!
//! 2026-10-16 下沉：见 follows.rs 头注。四个动作 black / unblack /
//! accept / reject 的落库语义全在这里，H 层只做口径判断。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};
use crate::repo::community::actor::actor_name;

/// 按用户名找活着的用户 id（status < 2 排除封禁/幽灵）。
pub async fn find_live_user_id(
    repo: &Repo,
    username: &str,
) -> DomainResult<Option<i64>> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM users WHERE username = $1 AND status < 2",
    )
    .bind(username)
    .fetch_optional(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .flatten())
}

/// 拉黑：upsert list='black'。
pub async fn set_black(repo: &Repo, me: i64, them: i64) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO friendships (user_id, friend_id, list) \
         VALUES ($1, $2, 'black') \
         ON CONFLICT (user_id, friend_id) DO UPDATE SET list = 'black'",
    )
    .bind(me)
    .bind(them)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 撤回我发给对方的 pending 申请（拉黑时一并做，避免两边同时挂着）。
pub async fn drop_my_pending(
    repo: &Repo,
    me: i64,
    them: i64,
) -> DomainResult<()> {
    sqlx::query(
        "DELETE FROM friendships WHERE user_id = $1 \
         AND friend_id = $2 AND list = 'pending'",
    )
    .bind(me)
    .bind(them)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 取消拉黑，返回受影响行数。
pub async fn unset_black(repo: &Repo, me: i64, them: i64) -> DomainResult<u64> {
    Ok(sqlx::query(
        "DELETE FROM friendships WHERE user_id = $1 \
         AND friend_id = $2 AND list = 'black'",
    )
    .bind(me)
    .bind(them)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}

/// 对方→我的 pending 申请是否存在（接受的前置判据）。
pub async fn has_their_pending(
    repo: &Repo,
    me: i64,
    them: i64,
) -> DomainResult<bool> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE user_id = $2 \
         AND friend_id = $1 AND list = 'pending')",
    )
    .bind(me)
    .bind(them)
    .fetch_one(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 把双向两行都翻成 'friend'。
pub async fn mark_both_friend(
    repo: &Repo,
    me: i64,
    them: i64,
) -> DomainResult<()> {
    sqlx::query(
        "UPDATE friendships SET list = 'friend' WHERE \
         (user_id = $1 AND friend_id = $2) \
         OR (user_id = $2 AND friend_id = $1)",
    )
    .bind(me)
    .bind(them)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 单向接受时我这一侧还没有行，补一条并置为 'friend'。
///
/// 好友关系必须是双向两行：好友列表 / 黑名单两侧都按 user_id=我 读，
/// 只 UPDATE 对方那一行会让我的列表里查不到这个人。
pub async fn ensure_my_side_friend(
    repo: &Repo,
    me: i64,
    them: i64,
) -> DomainResult<()> {
    sqlx::query(
        "INSERT INTO friendships (user_id, friend_id, list) \
         VALUES ($1, $2, 'friend') \
         ON CONFLICT (user_id, friend_id) \
         DO UPDATE SET list = 'friend'",
    )
    .bind(me)
    .bind(them)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 给对方发一条「申请已通过」系统信（best-effort，失败不阻断）。
pub async fn notify_friend_accepted(repo: &Repo, them: i64, me: i64) {
    let _ = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, kind) \
         VALUES (NULL, $1, $2, $3, 'friend_accepted')",
    )
    .bind(them)
    .bind("好友申请已通过")
    .bind(format!(
        "{} 接受了你的好友申请，你们现在是好友了。",
        actor_name(&repo.db, me).await
    ))
    .execute(&repo.db)
    .await;
}

/// 拒绝 = 只撤掉对方发来的那条申请，返回受影响行数。
pub async fn reject_their_pending(
    repo: &Repo,
    me: i64,
    them: i64,
) -> DomainResult<u64> {
    Ok(sqlx::query(
        "DELETE FROM friendships WHERE user_id = $2 \
         AND friend_id = $1 AND list = 'pending'",
    )
    .bind(me)
    .bind(them)
    .execute(&repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected())
}
