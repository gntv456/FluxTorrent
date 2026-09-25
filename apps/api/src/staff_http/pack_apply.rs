//! 站型包应用（向导）。
//! 从 staff_http.rs 按域拆出；物化事务体在 pack_core.rs（setup 向导与此端点共用，
//! 二审 G6：消除「向导只做半套 apply」的双口径）。

use super::sitetype::SiteTypePack;
use actix_web::{post, web, HttpRequest, Responder};

use super::pack_types::ApplyPackBody;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

/// 应用类型包（sysop）：重建分类 + 写 site_type/site_name + 更新模块开关
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
         sort, sections, tags, tagline, subtitle_kind, terms \
         FROM site_type_packs WHERE code = $1",
    )
    .bind(&body.code)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(pack) = pack else {
        return Err(DomainError::Validation("类型包不存在".into()));
    };

    // 登录页标语（0145 覆盖语义 / 二审 G7a 修复）：仅当当前值为空、或等于
    // 「任一预置包默认」时才重置——站长自定义值不再被 apply 无声清空。
    // 置空后 site-profile 会动态 JOIN 新站型包默认，即刻跟随新站型。
    sqlx::query(
        "UPDATE site_settings SET value = '', updated_at = now() \
         WHERE name = 'site_tagline' AND ( \
           value = '' OR value = ANY(ARRAY(SELECT tagline FROM site_type_packs \
             WHERE tagline IS NOT NULL AND tagline <> '')))",
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let (added, extras) =
        super::pack_core::apply_pack_full(&state.repo.db, &pack, mode).await?;
    // 术语段（0206）：NULL = 本包不声明 → 不动站方词汇表；数组 = 覆盖式重建。
    // 与 apply_pack_extras 一样落在主事务之外（都是「按包声明重建一张表」）。
    let terms_applied =
        super::pack_terms::apply_pack_terms(&state.repo.db, &pack).await?;
    // 模块开关进程缓存失效（apply 改 module_* 后立即生效，不等 30s TTL）
    state.module_flags.invalidate().await;
    state
        .repo
        .audit(Some(auth.id), "site_type_pack_apply", None)
        .await;
    Ok(ok(
        serde_json::json!({ "applied": pack.code, "mode": mode, "categories": added, "extras": extras,
            "terms_applied": terms_applied }),
    ))
}
