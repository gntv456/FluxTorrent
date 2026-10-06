//! 墓碑复活（0288，实测审计 P0-4）。
//!
//! 场景：作者删掉自己的种子（软删 `approval_status=3`，行与 info_hash 都保留），
//! 之后重发**同一枚 .torrent**。判重若不排除墓碑，或排除了但 INSERT 撞
//! `torrents_info_hash_key` 唯一索引，结果都一样——这条内容在本站**永久发不出去**
//! （实测：DELETE 成功后重发仍回「种子重复」）。而 .torrent 文件本身没有替换端点
//! （全仓 `torrent_files` 无 UPDATE），连「换个打包再发」都做不到。
//!
//! 口径：只有**同一作者**的墓碑可复活，复用原 id（`snatches`/评论/做种历史不悬空），
//! 内容字段按本次上传整体覆盖，清掉旧的拒绝标记与子表行后回到审核流。
//! 他人删过的墓碑不复活（报「种子重复」由调用方处理），避免「等别人删掉我再发」的上架漏洞。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::ptgen::UploadForm;

/// 复活后的种子状态：调用方按新上传的免审判定重设 approval_status。
#[allow(clippy::too_many_arguments)]
pub(super) async fn revive_tombstone(
    db: &PgPool,
    old_id: i64,
    form: &UploadForm,
    name: &str,
    parsed: &crate::bencode::ParsedTorrent,
    media_info: &Option<serde_json::Value>,
    nfo_text: &Option<String>,
    price: i64,
    imdb_id: &Option<String>,
    group_id: Option<i64>,
    approval_status: i16,
    screenshots: &[String],
) -> DomainResult<i64> {
    let n = sqlx::query(
        r#"UPDATE torrents
              SET info_hash = $2, raw_info_hash = $3, pieces_hash = $4,
                  group_id = $5, name = $6, small_descr = $7, descr = $8,
                  category_id = $9, medium_id = $10, grade_id = $11,
                  edition_id = $12, anonymous = $13, size = $14,
                  numfiles = $15, approval_status = $16, media_info = $17,
                  nfo = $18, price = $19, imdb_id = $20,
                  screenshots = $21::jsonb,
                  deny_reason_id = NULL, deny_note = NULL,
                  pos_state = 0, pos_state_until = NULL, pick_type = 0,
                  created_at = now(), mtime = now()
            WHERE id = $1 AND approval_status = 3"#,
    )
    .bind(old_id)
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
    .bind(&parsed.pieces_hash_hex)
    .bind(group_id)
    .bind(name)
    .bind(&form.small_descr)
    .bind(&form.descr)
    .bind(form.category_id)
    .bind(form.medium_id)
    .bind(form.grade_id)
    .bind(form.edition_id)
    .bind(form.anonymous)
    .bind(parsed.size)
    .bind(parsed.numfiles)
    .bind(approval_status)
    .bind(media_info)
    .bind(nfo_text)
    .bind(price)
    .bind(imdb_id)
    .bind(serde_json::json!(screenshots))
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        // 并发：墓碑被别人抢先恢复/清除。让调用方回落到正常判重路径。
        return Err(DomainError::Validation(
            "该内容的删除状态已变化，请重试".into(),
        ));
    }
    // 子表整体重来：旧的维度/标签/文件清单/原始字节都属于上一次发布
    for sql in [
        "DELETE FROM torrent_sections WHERE torrent_id = $1",
        "DELETE FROM tags WHERE torrent_id = $1",
        "DELETE FROM files WHERE torrent_id = $1",
        "DELETE FROM torrent_files WHERE torrent_id = $1",
    ] {
        sqlx::query(sql)
            .bind(old_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(old_id)
}
