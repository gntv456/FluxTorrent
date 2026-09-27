//! PUT /admin/settings/groups：分组批量保存 + 审计 + 缓存失效。
//! 从 settings_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{put, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use std::collections::HashMap;

use redis::AsyncCommands;

use super::engine::validate_field;
use super::meta::write_min;
use super::meta::MAX_BATCH;
use super::meta::{MetaRow, META_SELECT};

// PUT /admin/settings/groups —— 分组批量保存
// ============================================================================

#[derive(Deserialize)]
struct GroupsPut {
    /// 分区键（用于缓存失效与审计归类）；缺省时按首字段归属推断
    #[serde(default)]
    group: String,
    /// 字段名 -> 新值（secret 字段留空 = 保持不变）
    values: HashMap<String, String>,
}

pub(super) fn push_unique(out: &mut Vec<String>, s: &str) {
    if !out.iter().any(|x| x == s) {
        out.push(s.to_string());
    }
}

/// 关键字段变更后的影响提示（§4.2 effects）
pub(super) fn effects_for(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in names {
        match n.as_str() {
            "announce_url" | "https_announce_url" => push_unique(
                &mut out,
                "Tracker 地址已变更，已发布的旧种子文件需重新下载",
            ),
            "SITENAME" | "site_name" | "site_title" => push_unique(
                &mut out,
                "站点名称 / 标题变更，全站文案与 RSS 链接将更新",
            ),
            "BASEURL" => push_unique(
                &mut out,
                "站点根 URL 变更，RSS 与邮件内链接同步更新",
            ),
            "freeleech_until" => push_unique(
                &mut out,
                "全站免费窗口变更，对新 announce 立即生效",
            ),
            "destroy_disabled" | "deletepeasant" => {
                push_unique(&mut out, "危险操作开关已变更，随后台定时任务生效")
            }
            _ => {
                if n.starts_with("random")
                    || n.ends_with("become")
                    || n.starts_with("expire")
                {
                    push_unique(
                        &mut out,
                        "促销规则变更，新发布种子按新规则生效，已生效促销不受影响",
                    );
                }
                if n.starts_with("claim_") {
                    push_unique(
                        &mut out,
                        "保种认领规则变更，进行中的认领不受影响",
                    );
                }
                if n.starts_with("sticky_") {
                    push_unique(&mut out, "置顶样式变更，列表刷新后生效");
                }
            }
        }
    }
    out
}

/// 缓存失效：删分区缓存 + 广播变更（§7.2，不整库 FLUSHALL）
pub(super) async fn invalidate_cache(
    state: &web::Data<std::sync::Arc<AppState>>,
    group: &str,
) {
    let mut conn = state.redis.clone();
    let _: () = conn.del(format!("settings:{group}")).await.unwrap_or(());
    let _: () = conn.del("settings:all").await.unwrap_or(());
    // 模块开关（module_*）属 module 组：本进程缓存立即失效（U1 §5.1，最坏竞态由 TTL 兜底）
    state.module_flags.invalidate().await;
    // 0224 G30：settings:changed 的 publish 全仓零订阅者（否定式假通过），
    // 改走 cfg:ver 版本通道（订阅侧见 cfgver.rs；Redis 分区缓存 DEL 保留）
    crate::cfgver::bump(state.get_ref(), "modules").await;
}

#[put("/admin/settings/groups")]
pub(super) async fn settings_groups_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<GroupsPut>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    if body.values.is_empty() {
        return Err(DomainError::Validation("未提供任何修改".into()));
    }
    if body.values.len() > MAX_BATCH {
        return Err(DomainError::Validation(format!(
            "单次最多提交 {MAX_BATCH} 项"
        )));
    }

    let names: Vec<String> = body.values.keys().cloned().collect();
    let metas: Vec<MetaRow> =
        sqlx::query_as(&format!("{META_SELECT} WHERE s.name = ANY($1)"))
            .bind(&names)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let by_name: HashMap<String, MetaRow> =
        metas.into_iter().map(|m| (m.name.clone(), m)).collect();

    // 未知键直接拒绝（避免前端拼错导致静默丢配置）
    if let Some(missing) = names.iter().find(|n| !by_name.contains_key(*n)) {
        return Err(DomainError::Validation(format!("未知设定项 {missing}")));
    }
    // 三审 B-4：site_type 只能走站型包 apply（设置卡直改会让设置值与站点形态脱钩）
    if names.iter().any(|n| n == "site_type") {
        return Err(DomainError::Validation(
            "站点类型请在后台「分类管理 → 站点类型包」切换（会完整应用分类/模块/维度）".into(),
        ));
    }
    // 无任何可写字段 → 403（administrator 只读，§7.1）
    if !names.iter().any(|n| {
        !by_name[n].readonly && auth.class_id >= write_min(&by_name[n])
    }) {
        return Err(DomainError::Forbidden);
    }

    // ---- 逐字段校验（任一失败 → 字段级错误，整体不落库） ----
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<(String, String)> = Vec::new(); // (name, normalized_new)
    for name in &names {
        let meta = &by_name[name];
        let raw = &body.values[name];
        if meta.readonly {
            errors.push((name.clone(), "该字段为只读".into()));
            continue;
        }
        if auth.class_id < write_min(meta) {
            errors.push((name.clone(), "当前身份无权修改该字段".into()));
            continue;
        }
        if meta.secret && raw.trim().is_empty() {
            continue; // 密文留空 = 保持不变
        }
        match validate_field(meta, raw) {
            Ok(normalized) => {
                if normalized != meta.value {
                    pending.push((name.clone(), normalized));
                }
            }
            Err(e) => errors.push((name.clone(), e)),
        }
    }
    if !errors.is_empty() {
        return Err(DomainError::FieldErrors(errors));
    }

    // ---- 事务批量落库 + 审计（secret 不记明文） ----
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut changed: Vec<String> = Vec::new();
    for (name, new) in &pending {
        // 行锁 + 以库中现值为准（并发下避免覆盖他人修改）
        let current: Option<String> = sqlx::query_scalar(
            "SELECT value FROM site_settings WHERE name = $1 FOR UPDATE",
        )
        .bind(name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let current = current.ok_or_else(|| {
            DomainError::Validation(format!("设定项不存在 {name}"))
        })?;
        if current == *new {
            continue;
        }
        sqlx::query(
            "UPDATE site_settings SET value = $2, \
         updated_at = now() WHERE name = $1",
        )
        .bind(name)
        .bind(new)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let meta = &by_name[name];
        let (audit_old, audit_new) = if meta.secret {
            (
                if current.is_empty() {
                    "(空)".to_string()
                } else {
                    "已设置".to_string()
                },
                "已更新".to_string(),
            )
        } else {
            (truncate(&current), truncate(new))
        };
        sqlx::query(
            "INSERT INTO audit_log (id, actor_id, action, ref) \
             VALUES (nextval('audit_log_id_seq'), $1, 'setting:update', $2::jsonb)",
        )
        .bind(auth.id)
        .bind(
            serde_json::json!({
                "setting": name,
                "group": body.group,
                "old": audit_old,
                "new": audit_new,
            })
            .to_string(),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        changed.push(name.clone());
    }
    // 键收敛（0214 P2-4.6）：site_name 是权威键，SITENAME（NP 旧口径，RSS 频道
    // 名在读）随写同步——三个键此前各自为政，站长改一处另两处不动，RSS 与前台
    // 品牌名就分叉了。只镜像 site_name→SITENAME 方向；直接改 SITENAME 不回写
    // （旧键单改属罕见，且反向同步会在两键同改时产生写冲突）。
    if changed.iter().any(|n| n == "site_name") {
        if let Some((_, new)) = pending.iter().find(|(n, _)| n == "site_name") {
            sqlx::query(
                "UPDATE site_settings SET value = $2, updated_at = now() \
                 WHERE name = 'SITENAME' AND value IS DISTINCT FROM $2",
            )
            .bind("SITENAME")
            .bind(new)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // ---- 缓存失效 + 影响提示 ----
    let group = if body.group.trim().is_empty() {
        by_name[&names[0]].grp.clone()
    } else {
        body.group.trim().to_string()
    };
    if !changed.is_empty() {
        invalidate_cache(&state, &group).await;
    }
    let effects = effects_for(&changed);
    Ok(ok(serde_json::json!({
        "group": group,
        "saved": changed.len(),
        "changed": changed,
        "cache": format!("settings:{group}"),
        "effects": effects,
    })))
}

pub(super) fn truncate(s: &str) -> String {
    const MAX: usize = 500;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let head: String = s.chars().take(MAX).collect();
        format!("{head}…")
    }
}

// ============================================================================
