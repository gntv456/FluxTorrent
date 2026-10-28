//! 种子侧发布者正反馈通知（0423 P0-3）。
//! 种子被评论/被答谢此前零通知（论坛侧点赞/回复/提及全齐，种子侧双标）；
//! 发布是 PT 核心 UGC，「有人谢你」回路直接挂钩发布留存。
//! 两个助手都 best-effort：通知失败不影响主流程。

use sqlx::PgPool;

use crate::community_http::notify_user;

/// 读种主（只对过审种；匿名种 owner_id 为 NULL → None）。
/// 复用 thanks_spark::owner_of 的可见性口径但不报错——通知路径不抬 404。
pub(super) async fn visible_owner(db: &PgPool, torrent_id: i64) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT owner_id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
}

/// 有人评论了我的种子（评论是低频高价值反馈，免费通知不设槛）。
pub(super) async fn notify_comment(
    db: &PgPool,
    torrent_id: i64,
    commenter: i64,
) {
    let Some(owner) = visible_owner(db, torrent_id).await else {
        return;
    };
    if owner == commenter {
        return;
    }
    let uname: String =
        sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
            .bind(commenter)
            .fetch_one(db)
            .await
            .unwrap_or_else(|_| "user".into());
    notify_user(
        db,
        owner,
        "torrent_comment",
        "你的种子有新评论",
        &format!("{uname} 在种子 #{torrent_id} 下发表了评论，去看看吧。"),
    )
    .await;
}

/// 有人向我的种子答谢了魔力（只对 amount>0 的答谢发——免费感谢是高频
/// 按钮点击，热门种会轰炸种主信箱；此前答谢是「沉默到账」）。
pub(super) async fn notify_thanks(
    db: &PgPool,
    torrent_id: i64,
    thanker: i64,
    amount: i64,
) {
    let Some(owner) = visible_owner(db, torrent_id).await else {
        return;
    };
    if owner == thanker || amount <= 0 {
        return;
    }
    notify_user(
        db,
        owner,
        "torrent_thanks",
        "你的种子收到了答谢",
        &format!("种子 #{torrent_id} 收到了 {amount} 火花答谢，感谢你的发布。"),
    )
    .await;
}
