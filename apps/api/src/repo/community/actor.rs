//! 站内通知正文里「谁对我做了这事」的身份标签。
//!
//! 历史上发给会员的系统信有 12 处写成 `用户 #1 回复了你的主题`——收件人面对
//! 一个数字毫无判断依据（站内还有匿名发布者，探针号还同名成套），而这条信息
//! 本该一眼看懂是谁找上门。统一走这里的 `actor_name`。
//!
//! **例外（刻意保留 uid）**：发给管理组的处置工单与审计日志
//! （`community_http/leak_bot.rs`、前端 `classChanged`/`statusChanged` 等）——
//! 那是给职员按 id 操作的行，不是给会员读的句子。
//!
//! 落在 `repo/community/`（社区域仓储）：`repo/community/friends.rs` 与
//! `community_http/*` 两边都要用，而 `repo/mod.rs` 距 300 行门禁只剩 0 行、
//! 顶层再挂一个子模块就会变长；放在这里 `repo/mod.rs` 一行都不用改。

use sqlx::PgPool;

/// 展示名：在册用户给 username；被处置/注销（`status >= 2`，与鉴权层同口径：
/// 2 停用 / 3 墓碑 / 4 封禁）或查不到时给「已注销用户」。
///
/// 读库失败也回落到「已注销用户」而不是 uid：通知已经发出去了，正文里塞一个
/// `用户 #123` 比一个笼统的身份更难用。
pub async fn actor_name(db: &PgPool, uid: i64) -> String {
    sqlx::query_scalar(
        "SELECT CASE WHEN status < 2 THEN username END \
         FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_optional(db)
    .await
    .unwrap_or(None)
    .unwrap_or_else(|| "已注销用户".to_string())
}
