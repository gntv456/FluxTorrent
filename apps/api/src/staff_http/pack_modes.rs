//! 包 → 分类模式（0422）：站型包 apply 时按 `sections.kinds` 建一个同名
//! 模式，并把本包分类中**尚未绑定**的挂上去。
//!
//! 起因：`/section-dict?category_id=N` 的可见维度由 `mode_kinds` 决定，
//! 「该模式无 mode_kinds 行 = 全可见」（0195 兼容口径）。apply 此前只写
//! `section_kinds` / `section_dict`，从不写 `category_modes` ⇒ 任何分类
//! 都把全站 11 个站型的 42+ 维度铺进发布表单。本模块补「包自带维度集」
//! 这一层，与 0422 迁移的回填口径一致。
//!
//! 幂等与站长优先：
//!   · 模式按包名 upsert（同名复用），`mode_kinds` 整组重建（包是权威）；
//!   · 只绑 `mode_id IS NULL` 的分类——站长手工配过的分类不被切站型复位；
//!   · 包没声明 kinds（或全都不在注册表里）时不建模式，保持「全可见」。

use sqlx::{Postgres, Transaction};

use crate::errors::{DomainError, DomainResult};

use super::sitetype::SiteTypePack;

/// 旧 7 列镜像（保留期兼容：面板读 mode_kinds，旧消费点仍读这些列）。
/// 与 `admin_p3_http::sections::sync_legacy_mode_columns` 同口径。
const LEGACY_MIRROR_SQL: &str = "UPDATE category_modes m SET \
   show_source = EXISTS (SELECT 1 FROM mode_kinds k WHERE k.mode_id = m.id \
     AND k.visible AND k.kind = 'source'), \
   show_medium = EXISTS (SELECT 1 FROM mode_kinds k WHERE k.mode_id = m.id \
     AND k.visible AND k.kind = 'media'), \
   show_codec = EXISTS (SELECT 1 FROM mode_kinds k WHERE k.mode_id = m.id \
     AND k.visible AND k.kind = 'codec'), \
   show_audio_codec = EXISTS (SELECT 1 FROM mode_kinds k \
     WHERE k.mode_id = m.id AND k.visible AND k.kind = 'audio_codec'), \
   show_standard = EXISTS (SELECT 1 FROM mode_kinds k WHERE k.mode_id = m.id \
     AND k.visible AND k.kind = 'standard'), \
   show_processing = EXISTS (SELECT 1 FROM mode_kinds k \
     WHERE k.mode_id = m.id AND k.visible AND k.kind = 'processing'), \
   show_team = EXISTS (SELECT 1 FROM mode_kinds k WHERE k.mode_id = m.id \
     AND k.visible AND k.kind = 'team') \
 WHERE m.id = $1";

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

pub(crate) async fn apply_pack_modes(
    tx: &mut Transaction<'_, Postgres>,
    pack: &SiteTypePack,
) -> DomainResult<()> {
    let sections = pack.sections.as_ref();
    let kinds: Vec<String> = sections
        .and_then(|s| s.get("kinds"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.get("kind").and_then(|v| v.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let name = pack.name.trim();
    if kinds.is_empty() || name.is_empty() {
        return Ok(());
    }
    // 模式 upsert：同名复用（站长可能已手工配过同一模式）
    sqlx::query(
        "INSERT INTO category_modes (name) SELECT $1 \
         WHERE NOT EXISTS (SELECT 1 FROM category_modes WHERE name = $1)",
    )
    .bind(name)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    let mode_id: i32 = sqlx::query_scalar(
        "SELECT id FROM category_modes WHERE name = $1 ORDER BY id LIMIT 1",
    )
    .bind(name)
    .fetch_one(&mut **tx)
    .await
    .map_err(internal)?;
    // 可见维度整组重建；只收注册表里存在的 kind（包可能引用未安装维度）
    sqlx::query("DELETE FROM mode_kinds WHERE mode_id = $1")
        .bind(mode_id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    for kind in &kinds {
        sqlx::query(
            "INSERT INTO mode_kinds (mode_id, kind, visible) \
             SELECT $1, $2, TRUE \
              WHERE EXISTS (SELECT 1 FROM section_kinds WHERE kind = $2) \
             ON CONFLICT (mode_id, kind) DO UPDATE SET visible = TRUE",
        )
        .bind(mode_id)
        .bind(kind)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    }
    sqlx::query(LEGACY_MIRROR_SQL)
        .bind(mode_id)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    // 只绑尚未绑定的本包分类（key 是 0317 起的跨包稳定身份）
    let keys: Vec<String> = pack
        .categories
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|c| c.get("key").and_then(|v| v.as_str()))
                .filter(|k| !k.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if !keys.is_empty() {
        sqlx::query(
            "UPDATE categories SET mode_id = $1 \
             WHERE key = ANY($2) AND mode_id IS NULL",
        )
        .bind(mode_id)
        .bind(&keys)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    }
    Ok(())
}
