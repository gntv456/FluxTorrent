//! 站型包的标签数据节（0160 P2）：从 pack_apply.rs 拆出，守 300 行门禁。
//!
//! 词表两层：global（六件套：首发/官种/禁转/国语/中字/DIY）跨站型共享；
//! pack（站型专属：合集/带答案 等）随 apply 重建——未引用的删、在用的跳过
//! （宁残留不破坏，与 sections 维度清理同口径）。forum 域不受站型影响。

use super::sitetype::SiteTypePack;
use crate::errors::{DomainError, DomainResult};
use sqlx::{Postgres, Transaction};

/// 重建 pack 层标签词表；global 层硬性不触碰。
pub(super) async fn apply_pack_tags(
    tx: &mut Transaction<'_, Postgres>,
    pack: &SiteTypePack,
) -> DomainResult<()> {
    let packed_tags: Vec<(String, String, String)> = pack
        .tags
        .as_ref()
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|t| {
                    let name =
                        t.get("name").and_then(serde_json::Value::as_str)?;
                    let kind = match t
                        .get("kind")
                        .and_then(serde_json::Value::as_str)
                    {
                        Some("official") => "official",
                        _ => "plain",
                    };
                    let group = match t
                        .get("group")
                        .and_then(serde_json::Value::as_str)
                    {
                        Some("content") => "content",
                        _ => "attribute",
                    };
                    Some((
                        name.to_string(),
                        kind.to_string(),
                        group.to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    // pack 层中未被包定义且未被引用的 → 删（CASCADE 清关联行）；
    // 被引用的跳过（有种子的站点不丢筛选项）
    sqlx::query(
        "DELETE FROM tag_dict WHERE scope = 'torrent' \
         AND scope_layer = 'pack' AND name <> ALL($1) \
         AND NOT EXISTS (SELECT 1 FROM tags tg \
         WHERE tg.tag_id = tag_dict.id)",
    )
    .bind(
        packed_tags
            .iter()
            .map(|t| t.0.clone())
            .collect::<Vec<String>>(),
    )
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 包定义的标签 upsert（同名并入 pack 层——运营手工建的站型词被收编）
    for (name, kind, group) in &packed_tags {
        sqlx::query(
            "INSERT INTO tag_dict \
             (name, kind, scope, scope_layer, tag_group) \
             VALUES ($1, $2, 'torrent', 'pack', $3) \
             ON CONFLICT (name) DO UPDATE \
             SET scope = 'torrent', scope_layer = 'pack', tag_group = $3",
        )
        .bind(name)
        .bind(kind)
        .bind(group)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}
