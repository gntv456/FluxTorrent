//! 种子可见性唯一口径（0288 从 detail.rs 拆出，300 行门禁）。
//!
//! 实测审计 P0-3 / P1-1：被拒（approval_status=2）的种子原本对**作者和审核员都不可达**
//! ——而唯一的编辑入口长在详情页上，等于无人能自助；同时 files / snatches / peers /
//! comments / tags / collections 这些子资源各自只 `require_auth`，未过审内容的
//! 目录结构与下载者名单对全站任意登录用户可查。这里把两处收成一份判据。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 种子可见性的**唯一口径**（0288 收口，实测审计 P0-3 / P1-1）。
///
/// - `1` 过审：全员可读。
/// - `0` 待审 / `2` 被拒 / `4` 暂缓：仅**发布者本人与 staff**。
///   把 `2` 补进来是本轮的关键：旧口径只放 0/4，于是被拒的种子**连作者和审核员都读不到**
///   （实测 `GET /torrents/{id}`、`/detail`、`/download` 全 404），而唯一的编辑入口长在详情页上
///   ⇒ 谁都无法自助修改重提，只能开 ticket。
/// - `3` 软删：谁都不可读（治理面走 `/admin/torrents`）。
///
/// 所有读端点都必须走它。过去各子资源端点（files / snatches / peers / comments / tags /
/// collections）只 `require_auth` 后直查表，造成「主详情 404、子资源 200」的劈叉：
/// 待审与被拒种子的**文件目录结构**和**下载者名单**对全站任意登录用户可查。
pub fn visibility_sql(viewer: Option<(i64, bool)>) -> String {
    match viewer {
        Some((uid, is_staff)) => format!(
            "(t.approval_status = 1 OR (t.approval_status IN (0, 2, 4) \
             AND ({is_staff} OR t.owner_id = {uid})))"
        ),
        None => "t.approval_status = 1".to_string(),
    }
}

/// 子资源端点的统一准入门：口径与详情一致，不可见即 `NotFound`。
pub async fn assert_visible(
    db: &PgPool,
    id: i64,
    viewer: (i64, bool),
) -> DomainResult<()> {
    let vis = visibility_sql(Some(viewer));
    // EXISTS 而不是 `SELECT 1`：后者回 int4，按 i64 解会当场类型失配
    // （实测六个子资源端点全 500，闸门本身把可用面打挂了）
    let sql = "SELECT EXISTS(SELECT 1 FROM torrents t WHERE t.id = $1 AND "
        .to_owned()
        + &vis
        + ")";
    let hit: bool = sqlx::query_scalar(&sql)
        .bind(id)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if !hit {
        return Err(DomainError::NotFound(id));
    }
    Ok(())
}
