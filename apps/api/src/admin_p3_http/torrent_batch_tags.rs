//! 后台种子批量动作里的「打标」一支（从 `torrent_batch.rs` 拆出）。
//!
//! 为什么要拆出去：`torrent_batch.rs` 是存量超限件（基线 368 行，只许瘦不许胖），
//! 而 P-445 的修复要在 `set_tags` 分支里加存在性预筛——就地改会把行数顶过基线。
//! 拆法照仓里既有先例（`ops_http/preserve/claim.rs`、`torrents/reseed.rs`：
//! 认领/补种这种自成口径的一支单独一件）。
//!
//! P-445（2026-10-27 现网实测）：旧写法直接拿请求里的 id 逐颗 INSERT tags，
//! 遇到**不存在的种号**会撞 `tags.torrent_id → torrents.id` 外键，被
//! `DomainError::Internal` 折成 1000「内部错误」——而排在它前面的种子
//! **已经改完了**。界面只看到一句「服务异常」：分不清哪颗成、哪颗没成，
//! 重试还会给已经打过标的种子重复打标。实测两档：
//!   · `[好号, 坏号]` ⇒ HTTP 500，且好号那颗的 tags 行已落库（部分应用）
//!   · `[坏号]`       ⇒ HTTP 500
//! 现按本端点其他分支的同一把尺：不存在的种号算「没改动」（不抛 500），
//! 返回值是**真实处理颗数**——`admin-torrents-list.tsx:124` 那句
//! 「已应用 N 条」读的就是它，不能再拿请求条数充。
//!
//! 仍未收口的一半（如实说）：逐颗打标依然不在同一个事务里，所以如果
//! `apply_torrent_tags` 因**别的原因**（数据库抖动、标签字典被并发删）中途失败，
//! 依旧会留下「前几颗已成、后几颗没动」的半批。要彻底钉死得让
//! `apply_torrent_tags` 收 `&mut PgConnection`，牵动发布/详情多条写路径，
//! 属另一批工作；本批只把「不存在种号」这条最常见、且界面完全无感的触发源关掉。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 批量打标：先按存在性筛，再逐颗走与发布/详情同一套校验（0159）。
/// 返回真实处理的种子数（不存在的种号不计入，也不报错）。
pub(super) async fn set_tags(
    db: &PgPool,
    ids: &[i64],
    tag_ids: &[i32],
    actor: (i64, i16),
) -> DomainResult<u64> {
    let exist: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM torrents WHERE id = ANY($1) ORDER BY id",
    )
    .bind(ids)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for tid in &exist {
        crate::torrents::apply_torrent_tags(db, *tid, tag_ids, actor).await?;
    }
    Ok(exist.len() as u64)
}
