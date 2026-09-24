//! 种子编辑/删除的等级阈值（0159 用户反馈）：
//! 站点设定 `torrent_edit_class` / `torrent_delete_class`（数字 = user_classes.id），
//! 缺省 94（管理员）。语义：
//!  * class ≥ 阈值 → 可编辑/删除**任何人**的种子（管理通道）
//!  * 任何等级的**作者本人** → 可编辑自己的种子、删除自己未过审的种子（作者通道，
//!    与 NP 口径一致，不受阈值影响）
//! 站长把阈值调到 99 = 仅站长可管他人种子；调到 91 = 发布员即可。

use sqlx::PgPool;

pub(crate) const DEFAULT_EDIT_CLASS: i16 = 94;
pub(crate) const DEFAULT_DELETE_CLASS: i16 = 94;

async fn threshold(db: &PgPool, name: &str, default: i16) -> i16 {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.trim().parse::<i16>().ok())
    .filter(|v| (0..=99).contains(v))
    .unwrap_or(default)
}

/// 编辑他人的种子所需最低等级
pub(crate) async fn edit_threshold(db: &PgPool) -> i16 {
    threshold(db, "torrent_edit_class", DEFAULT_EDIT_CLASS).await
}

/// 删除他人的种子所需最低等级
pub(crate) async fn delete_threshold(db: &PgPool) -> i16 {
    threshold(db, "torrent_delete_class", DEFAULT_DELETE_CLASS).await
}

/// 发布免审最低等级（0170）：站点设定 `upload_auto_approve_class`，缺省 92（论坛版主）。
/// 语义：class ≥ 阈值的用户发布种子免审直接通过（upload.rs 免审链第 4 条）；
/// 该等级的作者编辑自己的种子也不再回退待审（interact.rs 编辑回退联动）。
pub(crate) const DEFAULT_AUTO_APPROVE_CLASS: i16 = 92;

pub(crate) async fn auto_approve_threshold(db: &PgPool) -> i16 {
    threshold(
        db,
        "upload_auto_approve_class",
        DEFAULT_AUTO_APPROVE_CLASS,
    )
    .await
}
