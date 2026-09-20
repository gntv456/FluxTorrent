//! 发种多维属性与标签入库（M04 第八轮 Section）。
//! 从 publish_http/upload.rs 按域拆出；校验 kind 白名单后写 torrent_sections + tags。

use actix_web::web;

use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;
use crate::state::AppState;

use super::ptgen::UploadForm;

pub(super) async fn store_sections_tags(
    state: &web::Data<std::sync::Arc<AppState>>,
    form: &UploadForm,
    auth: &AuthUser,
    id: i64,
) -> DomainResult<()> {
    // 多维属性（第八轮 Section）：校验 kind 白名单后写 torrent_sections
    if let Some(json) = form
        .sections
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    {
        let map: std::collections::HashMap<String, i64> =
            serde_json::from_str(json).map_err(|_| {
                DomainError::Validation("sections 需为 JSON 对象".into())
            })?;
        for (kind, dict_id) in &map {
            // 0085/0087：维度可由站方自建（含 media/grades/editions），白名单查 section_kinds
            if !crate::admin_p3_http::is_custom_kind(&state.repo.db, kind).await
            {
                return Err(DomainError::Validation(format!(
                    "未知维度 {kind}"
                )));
            }
            // 字典归属校验：dict_id 必须属于该 kind（防跨维度错挂）
            let ok: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM section_dict WHERE id = $2 AND kind = $1)",
                )
                .bind(kind)
                .bind(dict_id)
                .fetch_one(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            if !ok {
                return Err(DomainError::Validation(format!(
                    "维度 {kind} 的字典项 {dict_id} 不存在"
                )));
            }
            sqlx::query(
                    "INSERT INTO torrent_sections (torrent_id, kind, dict_id) VALUES ($1, $2, $3)                  ON CONFLICT (torrent_id, kind) DO UPDATE SET dict_id = EXCLUDED.dict_id",
                )
                .bind(id)
                .bind(kind)
                .bind(dict_id)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }

    // 标签（NP upload.php tags 口径）：发布时直接打标；启用字典校验 + 官方标签仅 staff
    if let Some(json) = form
        .tags
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    {
        let ids: Vec<i32> = serde_json::from_str(json).map_err(|_| {
            DomainError::Validation("tags 需为 JSON 数组".into())
        })?;
        if ids.len() > 12 {
            return Err(DomainError::Validation("标签最多选择 12 个".into()));
        }
        let is_staff = auth.class_id >= 90;
        for tid in &ids {
            let row: Option<(String, bool)> = sqlx::query_as(
                "SELECT kind, COALESCE(enabled, TRUE) FROM tag_dict \
                     WHERE id = $1 AND scope = 'torrent'",
            )
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some((kind, enabled)) = row else {
                return Err(DomainError::Validation(format!(
                    "标签 {tid} 不存在"
                )));
            };
            if !enabled {
                return Err(DomainError::Validation(format!(
                    "标签 {tid} 已停用"
                )));
            }
            if kind == "official" && !is_staff {
                return Err(DomainError::Forbidden); // 与详情页打标同口径
            }
            sqlx::query(
                    "INSERT INTO tags (torrent_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
                )
                .bind(id)
                .bind(tid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    Ok(())
}
