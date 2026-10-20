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

#[derive(Deserialize, utoipa::ToSchema)]
pub(super) struct BatchReq {
    ids: Vec<i64>,
}

#[utoipa::path(
    post,
    path = "/api/v1/torrents/batch-download",
    tag = "种子",
    description = "批量下载（一次下发多个 .torrent；仍逐条过下载闸门与扣费）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）"),
    )
)]
#[post("/torrents/batch-download")]
pub(crate) async fn batch_download(
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

    // 资格闸门整单只判一次，且必须在扣费循环之前（P-162）：被停下载/挂起
    // 的账号此前会把选中的付费种逐个扣完（实测上限 50 × 价）才发现一个都
    // 发不出去 —— 已购流水留下，重试还会跳过扣费段，钱回不来。
    torrents::ensure_downloadable(&state.repo.db, auth.id).await?;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut skipped: usize = 0;
    // P-432（批二百零六）：先发货，后动账。旧顺序把 charge_for_download 排在
    // build_torrent_bytes 之前，而循环里没有任何退款路径——付费种 + 原始
    // .torrent 存档丢失时，魔力当场扣掉、「已购」流水写好，文件却拿不到，
    // 重试还因已购跳过扣费段（P-162「闸门必须在扣费前」被「发货能力」这一维
    // 绕过）。现在字节已在手里才扣费，扣成功立刻进 zip，中间没有可失败点。
    for id in &ids {
        let bytes = match build_torrent_bytes(&state, auth.id, *id).await {
            Ok(b) => b,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if torrents::charge_for_download(&state.repo.db, auth.id, *id)
            .await
            .is_err()
        {
            // 扣费没成（余额不足 / 种不存在 / 未过审）：丢掉已取字节，不发
            skipped += 1;
            continue;
        }
        let label = names
            .get(id)
            .cloned()
            .unwrap_or_else(|| format!("torrent-{id}"));
        let clean = archive::safe_name(&label);
        files.push((format!("{clean}-{id}.torrent"), bytes));
    }
    if files.is_empty() {
        // 一个都没成功：报「都没能打包」并带件数。旧写法回
        // NotFound(ids[0])，把一枚存在且过审的种（只是没存档）说成
        // 「资源不存在」，与单种口的 400「种子文件缺失…请联系管理组补档」
        // 口径相反（HX2/HX4 钉的那条）。
        return Err(DomainError::Validation(format!(
            "{skipped} 个种子都没能打包（原始 .torrent 未存档、已下架，\
             或余额不足），请联系管理组"
        )));
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
    // zip 内每个 .torrent 都内嵌本人 announce+passkey：与单种下载同口径
    // 禁缓存（P-163 的同一条不变式）
    Ok(crate::openapi_http::no_store(resp.body(zip)))
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
