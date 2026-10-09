//! 种子工件存储（0323 game/software 批：校验和清单/更新日志）。
//!
//! 与 logcheck 的 log part 同范式：multipart 里 `artifact` part 走
//! 独立文本通道（不被 256 KiB 字段上限截断），kind 由 filename 后缀
//! 推断（.sfv/.md5/.sha1 → checksums；changelog/news → changelog；
//! license → license），也可用元数据字段 `artifact_kind` 显式指定
//! （多个工件按 part 顺序对应，超出 kind 数的回落文件名推断）。
//! 更新链：元数据字段 `parent_torrent_id` 把本种挂到本体（GGn GameDOX
//! 语义），由 upload 主流程透传到这里。

use actix_web::web::Bytes;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// 单个工件的体积上限（校验和清单/日志是纯文本，256 KiB 足够大）
pub(super) const ARTIFACT_MAX: usize = 256 * 1024;

/// 单次发种最多带的工件数（防一个请求塞几十个 part）
pub(super) const ARTIFACT_MAX_COUNT: usize = 8;

#[derive(Debug, Clone)]
pub(super) struct ArtifactPart {
    pub filename: String,
    pub kind: String,
    pub body: String,
}

/// multipart `artifact` part 的收集回调（upload_body.rs 调用）。
/// kind 推断：文件名小写包含后缀/关键词 → 缺省 other。
pub(super) fn infer_kind(filename: &str) -> &'static str {
    let low = filename.to_ascii_lowercase();
    if low.ends_with(".sfv")
        || low.ends_with(".md5")
        || low.ends_with(".sha1")
        || low.ends_with(".sha256")
        || low.contains("checksum")
    {
        "checksums"
    } else if low.ends_with(".cue") {
        // cue.rs 的存量取证回落口查 kind='cue' 的工件——缺这一支
        // 「发种时上传 .cue」永远落不进该通道（0337 HasCue 断链）
        "cue"
    } else if low.contains("changelog")
        || low.contains("update")
        || low.contains("更新")
    {
        "changelog"
    } else if low.contains("license") || low.contains("licence") {
        "license"
    } else {
        "other"
    }
}

/// 落库（发种成功路径，upload.rs 在拿到种子 id 后调用）。
/// `parent`：更新包挂链的本体种子 id（None = 自身即本体）。
pub(super) async fn store(
    db: &PgPool,
    torrent_id: i64,
    parent: Option<i64>,
    parts: &[ArtifactPart],
) -> DomainResult<u64> {
    if parts.is_empty() {
        return Ok(0);
    }
    // 本体必须存在且不是自己（防自引用环）
    let parent = match parent {
        Some(p) if p != torrent_id => {
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM torrents WHERE id = $1)",
            )
            .bind(p)
            .fetch_one(db)
            .await
            .unwrap_or(false);
            ok.then_some(p)
        }
        _ => None,
    };
    let mut n = 0u64;
    for a in parts {
        let res = sqlx::query(
            "INSERT INTO torrent_artifacts \
             (torrent_id, parent_torrent_id, kind, filename, body, \
              sha256, size_bytes) \
             VALUES ($1, $2, $3, $4, $5, \
             encode(digest($5, 'sha256'), 'hex'), $6) \
             ON CONFLICT (torrent_id, kind, filename) DO NOTHING",
        )
        .bind(torrent_id)
        .bind(parent)
        .bind(&a.kind)
        .bind(&a.filename)
        .bind(&a.body)
        .bind(a.body.len() as i64)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        n += res.rows_affected();
    }
    Ok(n)
}

/// 详情聚合的 artifacts 段（torrent_http/aggregate.rs 消费）。
/// 不带 body（列表语义）；正文由独立端点懒加载。
pub(crate) async fn list_for_detail(
    db: &PgPool,
    torrent_id: i64,
) -> Vec<serde_json::Value> {
    let rows: Vec<(i64, Option<i64>, String, String, Option<String>, i64)> =
        sqlx::query_as(
            "SELECT id, parent_torrent_id, kind, filename, sha256, \
             size_bytes FROM torrent_artifacts WHERE torrent_id = $1 \
             ORDER BY kind, id",
        )
        .bind(torrent_id)
        .fetch_all(db)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(|(id, parent, kind, filename, sha, size)| {
            serde_json::json!({
                "id": id, "parent_torrent_id": parent, "kind": kind,
                "filename": filename, "sha256": sha, "size": size,
            })
        })
        .collect()
}

/// 更新链导航：挂在「本体」下的更新包列表（详情页「更新」段）。
pub(crate) async fn children_of(
    db: &PgPool,
    parent_id: i64,
) -> Vec<serde_json::Value> {
    let rows: Vec<(i64, i64, String, Option<String>, i64)> = sqlx::query_as(
        "SELECT a.torrent_id, t.id, t.name, \
         to_char(t.created_at, 'YYYY-MM-DD HH24:MI'), t.size \
         FROM torrent_artifacts a JOIN torrents t ON t.id = a.torrent_id \
         WHERE a.parent_torrent_id = $1 ORDER BY t.created_at DESC LIMIT 20",
    )
    .bind(parent_id)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .map(|(aid, tid, name, created, size)| {
            serde_json::json!({
                "torrent_id": tid, "name": name,
                "created_at": created, "size": size,
                "via_artifact": aid,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_inference() {
        assert_eq!(infer_kind("setup.sfv"), "checksums");
        assert_eq!(infer_kind("game.md5"), "checksums");
        assert_eq!(infer_kind("game.sha256"), "checksums");
        assert_eq!(infer_kind("checksums.sum"), "checksums");
        assert_eq!(infer_kind("changelog_1.2.txt"), "changelog");
        assert_eq!(infer_kind("license.txt"), "license");
        assert_eq!(infer_kind("misc.bin"), "other");
    }
}
