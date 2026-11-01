//! 作品键（WK）按种子求值（0450）：追更订阅用。
//!
//! WK 是 `works_sql.rs` 里那条作品聚合表达式。订阅派发要「这颗种属于哪部
//! 作品」，而它在 admin_http 侧 —— 让那边自己抄一份表达式必然漂移（本仓
//! 反复抓到的病形），所以这里开个窄口：只暴露「按种子 id 求 WK」。
//!
//! 不加 approval_status 谓词：作品键与可见性无关，未过审的种子也属于同一部
//! 作品；订阅要能覆盖「作品刚出现的这一刻」。

use sqlx::PgPool;

use super::works_sql::WK;

/// 按种子 id 求作品键；种子不存在或表达式求值失败返回 None。
pub(crate) async fn work_key_of(
    db: &PgPool,
    torrent_id: i64,
) -> Option<String> {
    sqlx::query_scalar(&format!("SELECT {WK} FROM torrents t WHERE t.id = $1"))
        .bind(torrent_id)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
}
