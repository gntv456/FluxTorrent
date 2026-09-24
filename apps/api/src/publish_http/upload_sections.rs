//! 发种多维属性与标签入库（M04 第八轮 Section）。
//! 从 publish_http/upload.rs 按域拆出；校验 kind 白名单后写 torrent_sections + tags。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;
use crate::state::AppState;

use super::ptgen::UploadForm;

/// 解析并**整体**校验 sections：任一维度/选项非法就一条都不写。
///
/// 发种主链必须在 `INSERT torrents` 之前先调它。原先校验与写入交织在同一个循环里，
/// 后一个维度报错时种子已经入库、前面的归属也已落库——用户只看到一个 400，
/// 而重试同一个 .torrent 会永远撞 TorrentDuplicate。
pub(super) async fn parse_sections(
    db: &sqlx::PgPool,
    raw: Option<&String>,
) -> DomainResult<std::collections::HashMap<String, i64>> {
    let json = match raw.map(|s| s.as_str().trim()).filter(|s| !s.is_empty()) {
        Some(j) => j,
        None => return Ok(Default::default()),
    };
    let map: std::collections::HashMap<String, i64> =
        serde_json::from_str(json).map_err(|_| {
            DomainError::Validation("sections 需为 JSON 对象".into())
        })?;
    for (kind, dict_id) in &map {
        // 0085/0087：维度可由站方自建，白名单查 section_kinds
        if !crate::admin_p3_http::is_custom_kind(db, kind).await {
            return Err(DomainError::Validation(format!("未知维度 {kind}")));
        }
        // dict_id 必须属于该 kind：外键只保证这行字典存在，不保证没挂错维度
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM section_dict WHERE id = $1 AND \
             kind = $2)",
        )
        .bind(dict_id)
        .bind(kind)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if !ok {
            return Err(DomainError::Validation(format!(
                "维度 {kind} 的字典项 {dict_id} 不存在"
            )));
        }
    }
    Ok(map)
}

pub(super) async fn store_sections_tags(
    state: &web::Data<std::sync::Arc<AppState>>,
    form: &UploadForm,
    auth: &AuthUser,
    id: i64,
) -> DomainResult<()> {
    // 多维属性（第八轮 Section）：parse_sections 已整体校验，这里只写
    for (kind, dict_id) in
        parse_sections(&state.repo.db, form.sections.as_ref()).await?
    {
        sqlx::query(
            "INSERT INTO torrent_sections \
             (torrent_id, kind, dict_id) VALUES ($1, $2, $3) ON \
             CONFLICT (torrent_id, kind) DO UPDATE SET dict_id = \
             EXCLUDED.dict_id",
        )
        .bind(id)
        .bind(&kind)
        .bind(dict_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 标签（NP upload.php tags 口径）：发布时直接打标；统一走 apply_torrent_tags
    // （0159：校验 + official_tag 联动三入口同源，官种物化列不再漂移）
    if let Some(json) = form
        .tags
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    {
        let ids: Vec<i32> = serde_json::from_str(json).map_err(|_| {
            DomainError::Validation("tags 需为 JSON 数组".into())
        })?;
        crate::torrents::apply_torrent_tags(
            &state.repo.db,
            id,
            &ids,
            (auth.id, auth.class_id as i16),
        )
        .await?;
    }
    Ok(())
}
