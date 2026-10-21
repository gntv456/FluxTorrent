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
///
/// **交付性口径 = RSS/Torznab（P-433）同一把尺**：只回有存档的种。
/// 实测（批二百四十，现网）：7 个「已过审但无 `.torrent` 存档」的种都能被
/// 指纹命中，客户端拿到的 `download_url` 一打就是 400（`issue_download_key`
/// 那三道闸之外还有一道存档判定）——链接根本兑不了现，还要按 10 次/分钟
/// 吃掉辅种下载额度。辅种是对外的第三个出口，不该比 RSS 宽。
/// 共用 `torrents::HAS_ARCHIVE_CLAUSE` 而不是自己抄一份，防的是
/// 「存档判定分叉」——那条常量已经因为同样原因修过一次。
pub async fn find_by_fingerprints(
    repo: &Repo,
    info_hash_v1: &[String],
    pieces_sha1: &[String],
) -> Result<Vec<HitRow>, sqlx::Error> {
    sqlx::query_as::<_, HitRow>(&format!(
        "SELECT t.id, t.name, t.size, t.info_hash, t.pieces_hash FROM \
         torrents t WHERE t.approval_status = 1 AND {arch} \
           AND (t.info_hash = ANY($1::char(40)[]) \
                OR t.pieces_hash = ANY($2::char(40)[]))",
        arch = crate::torrents::HAS_ARCHIVE_CLAUSE
    ))
    .bind(info_hash_v1)
    .bind(pieces_sha1)
    .fetch_all(&repo.db)
    .await
}
