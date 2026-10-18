//! 辅种查询的 SQL 唯一居所（FRAME §2 H 层下沉）。
//!
//! 消费方：openapi_http/crossseed/{mod,iyuu}.rs。**两份外壳一份内核**——
//! `/crossseed/lookup` 与 `/crossseed/iyuu` 都调 `find_by_fingerprints`，
//! 可见性口径（只回已审核种）与双列 ANY 的查询形状只有这一处，
//! 杜绝「两个出口两套口径」那种漂移。
//!
//! H 层只留：指纹归一化（norm_hashes）、限流/配额、响应组装。

use super::Repo;

/// 命中行：id / 名称 / 体积 / v1 info_hash / pieces_sha1。
///
/// `pieces_hash` 可为 NULL（历史数据未回填），此时上层按 info_hash 归类。
pub type HitRow = (i64, String, i64, String, Option<String>);

/// 双指纹批量命中：v1 列与 pieces 列各一次 ANY（两列都有索引）。
///
/// 可见性口径 = 详情页：`approval_status = 1`（已过审）。未过审的种
/// 不进辅种结果——否则会通过辅种链泄露待审内容的存在性。
pub async fn find_by_fingerprints(
    repo: &Repo,
    info_hash_v1: &[String],
    pieces_sha1: &[String],
) -> Result<Vec<HitRow>, sqlx::Error> {
    sqlx::query_as::<_, HitRow>(
        "SELECT id, name, size, info_hash, pieces_hash FROM torrents \
         WHERE approval_status = 1 \
           AND (info_hash = ANY($1::char(40)[]) \
                OR pieces_hash = ANY($2::char(40)[]))",
    )
    .bind(info_hash_v1)
    .bind(pieces_sha1)
    .fetch_all(&repo.db)
    .await
}
