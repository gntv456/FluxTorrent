//! 批量下载（阶段三「最强」：把选中的种子打成一个 zip 下发）。
//!
//! 逐项复用单种下载的既有链路：`charge_for_download`（只对付费种扣费，
//! 免费种/自己发布的直接放行）+ `build_torrent_bytes`（下载闸门校验 +
//! 站点 announce/passkey 注入）。任何一项失败只跳过该项，不整单失败——
//! 批量场景下「部分可用」比「全有全无」合用；但一个都没成时返回错误。
//! 跳过数通过 `X-Batch-Skipped` 头回传，前端据此提示。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::archive;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::publish_http::build_torrent_bytes;
use crate::state::AppState;
use crate::torrents;

/// 单次打包上限（zip 在内存里拼，50 个 .torrent ≈ 几 MB，安全）
const MAX_BATCH: usize = 50;

#[derive(Deserialize)]
pub(super) struct BatchReq {
    ids: Vec<i64>,
}

#[post("/torrents/batch-download")]
async fn batch_download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BatchReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 去重（保序）+ 丢弃非法 id
    let mut ids: Vec<i64> = Vec::new();
    for id in &body.ids {
        if *id > 0 && !ids.contains(id) {
            ids.push(*id);
        }
    }
    if ids.is_empty() {
        return Err(DomainError::Validation("未选择任何种子".into()));
    }
    if ids.len() > MAX_BATCH {
        return Err(DomainError::Validation(format!(
            "单次最多打包 {MAX_BATCH} 个种子"
        )));
    }
    // 种子名进 zip 条目名（安全清洗后）；查不到的 id 会走 build 失败分支
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, name FROM torrents WHERE id = ANY($1)")
            .bind(&ids)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let names: std::collections::HashMap<i64, String> =
        rows.into_iter().collect();

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut skipped: usize = 0;
    for id in &ids {
        if torrents::charge_for_download(&state.repo.db, auth.id, *id)
            .await
            .is_err()
        {
            skipped += 1;
            continue;
        }
        match build_torrent_bytes(&state, auth.id, *id).await {
            Ok(bytes) => {
                let label = names
                    .get(id)
                    .cloned()
                    .unwrap_or_else(|| format!("torrent-{id}"));
                let clean = archive::safe_name(&label);
                files.push((format!("{clean}-{id}.torrent"), bytes));
            }
            Err(_) => skipped += 1,
        }
    }
    if files.is_empty() {
        // 一个都没成功：多半是权限/闸门问题，交给调用方看到 404
        return Err(DomainError::NotFound(ids[0]));
    }
    let count = files.len();
    let zip = archive::zip_stored(&files);
    let mut resp = HttpResponse::Ok();
    resp.insert_header(("Content-Type", "application/zip"));
    // 文件名保持纯 ASCII（中文需 RFC 5987 编码，这里没必要）
    resp.insert_header((
        "Content-Disposition",
        format!("attachment; filename=\"fluxtorrent-{count}items.zip\""),
    ));
    resp.insert_header(("X-Batch-Skipped", skipped.to_string()));
    Ok(resp.body(zip))
}

#[cfg(test)]
mod tests {
    use super::MAX_BATCH;

    /// 上限是接口契约的一部分（前端按它做校验提示），改动要同步前端
    #[test]
    fn batch_limit_is_50() {
        assert_eq!(MAX_BATCH, 50);
    }
}
