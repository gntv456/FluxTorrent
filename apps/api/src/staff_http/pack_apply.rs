//! 站型包应用（向导）。
//! 从 staff_http.rs 按域拆出。

use super::sitetype::SiteTypePack;
use actix_web::{post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 应用类型包（sysop）：重建分类 + 写 site_type/site_name + 更新课本模块开关
#[post("/admin/site-type-packs/apply")]
pub async fn site_type_pack_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ApplyPackBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let mode = body.mode.as_deref().unwrap_or("replace");
    if !["replace", "merge"].contains(&mode) {
        return Err(DomainError::Validation("mode 需为 replace/merge".into()));
    }
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, \
         sort, sections, tagline FROM site_type_packs WHERE code = $1",
    )
    .bind(&body.code)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(pack) = pack else {
        return Err(DomainError::Validation("类型包不存在".into()));
    };

    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let cats = pack.categories.as_array().cloned().unwrap_or_default();
    let added = cats.len() as i64;
    if mode == "replace" {
        let used: i64 = sqlx::query_scalar("SELECT count(*) FROM torrents")
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
        if used > 0 {
            // 有种子时禁止整表重建（避免悬挂引用）：提示改用 merge
            return Err(DomainError::Validation(
                "站点已有种子，replace 会悬挂引用；请使用 merge 模式（保留现有分类，追加新分类）"
                    .into(),
            ));
        }
        sqlx::query("DELETE FROM categories")
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    for (i, c) in cats.iter().enumerate() {
        let id = c
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(i as i64 + 1) as i32;
        let name = c
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        let _ = sqlx::query(
            "INSERT INTO categories (id, name) VALUES ($1, $2) ON \
             CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",
        )
        .bind(id)
        .bind(&name)
        .execute(&mut *tx)
        .await;
    }
    // site_type + 品牌默认
    sqlx::query("INSERT INTO site_settings (name, value) VALUES \
     ('site_type', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
     updated_at = now()")
        .bind(&pack.code).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("INSERT INTO site_settings (name, value) VALUES \
     ('site_name', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
     updated_at = now()")
        .bind(&pack.brand).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 登录页标语（0145）：不再物化包默认值——切站型后 site_tagline 保持空（覆盖
    // 语义），site-profile 动态 JOIN 新站型包默认即刻生效；站长自定义值也被保留，
    // 不会被下一次 apply 无声重置
    sqlx::query(
        "INSERT INTO site_settings (name, value) VALUES \
     ('site_tagline', '') ON CONFLICT (name) DO UPDATE SET value = '', \
     updated_at = now()",
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 模块开关 → 站点设定键（textbooks 等）
    if let Some(mods) = pack.modules.as_object() {
        for (k, v) in mods {
            let val = if v.as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            let _ = sqlx::query(
                "INSERT INTO site_settings (name, value) \
                 VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET value = \
                 EXCLUDED.value, updated_at = now()",
            )
            .bind(format!("module_{k}"))
            .bind(val)
            .execute(&mut *tx)
            .await;
        }
    }
    // 质量维度种子（0092）：包内定义的维度重建标签与选项（references 级联清理旧引用）。
    // 0101 修复：切换站型后旧站型的内置维度残留（切音乐站仍见「游戏类型」）——
    // 内置九维中未被本包定义的维度整体移除（section_kinds 级联清 section_dict 与
    // torrent_sections 引用）；站方自建维度（不在内置清单）原样保留。
    let builtin: std::collections::HashSet<String> = [
        "media",
        "grades",
        "editions",
        "codec",
        "audio_codec",
        "standard",
        "source",
        "processing",
        "team",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut packed_kinds: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    if let Some(sections) = pack
        .sections
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        if let Some(kinds) =
            sections.get("kinds").and_then(serde_json::Value::as_array)
        {
            for k in kinds {
                if let Some(kind) =
                    k.get("kind").and_then(serde_json::Value::as_str)
                {
                    packed_kinds.insert(kind.to_string());
                }
            }
        }
    }
    for kind in &builtin {
        if !packed_kinds.contains(kind) {
            // 引用中的维度直接删会级联清 torrent_sections —— 有种子的站点会丢筛选项，
            // 这里先检查是否被在用：被在用时跳过清理（宁残留不破坏）
            let in_use: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM torrent_sections WHERE kind = $1",
            )
            .bind(kind)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
            if in_use == 0 {
                sqlx::query("DELETE FROM section_kinds WHERE kind = $1")
                    .bind(kind)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
    }
    if let Some(sections) = pack
        .sections
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        if let Some(kinds) =
            sections.get("kinds").and_then(serde_json::Value::as_array)
        {
            for k in kinds {
                let (Some(kind), Some(label)) = (
                    k.get("kind").and_then(serde_json::Value::as_str),
                    k.get("label").and_then(serde_json::Value::as_str),
                ) else {
                    continue;
                };
                if !is_ascii_kind(kind) {
                    continue;
                }
                let sort = k
                    .get("sort")
                    .and_then(serde_json::Value::as_i64)
                    .unwrap_or(999) as i32;
                sqlx::query(
                    "INSERT INTO section_kinds (kind, label, sort) VALUES ($1, $2, $3) \
                     ON CONFLICT (kind) DO UPDATE SET label = EXCLUDED.label, sort = EXCLUDED.sort",
                )
                .bind(kind)
                .bind(label)
                .bind(sort)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
        if let Some(dict) =
            sections.get("dict").and_then(serde_json::Value::as_object)
        {
            for (kind, names) in dict {
                if !is_ascii_kind(kind) {
                    continue;
                }
                // 维度可能未在包 kinds 中定义（自定义维度追加选项）：确保存在
                sqlx::query(
                    "INSERT INTO section_kinds (kind, \
                     label, sort) VALUES ($1, $1, 999) ON CONFLICT (kind) DO \
                     NOTHING",
                )
                .bind(kind)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                // 整维替换：旧选项与其 torrent_sections 引用级联清除（显式应用包 = 重建口径）
                sqlx::query("DELETE FROM section_dict WHERE kind = $1")
                    .bind(kind)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                for (i, name) in names
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    let Some(name) = name.as_str() else { continue };
                    sqlx::query(
                        "INSERT INTO section_dict \
                     (kind, name, sort) VALUES ($1, $2, $3)",
                    )
                    .bind(kind)
                    .bind(name)
                    .bind((i + 1) as i32)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // U2（0108）：等级叙事/经济预设/元数据源（apply_pack_extras 过程内含合法键校验）
    let extras: Vec<(String, i64)> =
        sqlx::query_as("SELECT kind, applied FROM apply_pack_extras($1)")
            .bind(&pack.code)
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
    // 模块开关进程缓存失效（apply 改 module_* 后立即生效，不等 30s TTL）
    state.module_flags.invalidate().await;
    state
        .repo
        .audit(Some(auth.id), "site_type_pack_apply", None)
        .await;
    Ok(ok(
        serde_json::json!({ "applied": pack.code, "mode": mode, "categories": added, "extras": extras }),
    ))
}

#[derive(Deserialize)]
pub(super) struct ApplyPackBody {
    pub(super) code: String,
    /// replace = 清空现有分类重建；merge = 保留现有，仅追加新分类
    #[serde(default)]
    pub(super) mode: Option<String>,
}

/// 维度 kind 合法性（防注入）：小写字母/数字/下划线
pub(super) fn is_ascii_kind(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
