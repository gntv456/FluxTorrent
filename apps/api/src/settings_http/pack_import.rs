//! 内容包导入与回滚的 HTTP 编排（M1 核心）：快照 → 纯覆盖落库 → 登记 → 审计。
//! 从 packs.rs 按域拆出；格式在 pack_format.rs，落库原语在 pack_store.rs。
//! 路径纪律：导入（confirm=true）在**单事务**内：拍快照 → 防悬挂检查 →
//! 落库 → 写 content_packs → 审计 → 提交，失败整体回退；回滚 = 读 snapshot
//! 包装成「逆向导入」重放同一纯覆盖路径（永远可用），登记行删除。
//! - 预检（confirm=false）只读：重名分类悬挂检查 + theme 白名单/校验 + 差异计数。

use actix_web::{web, HttpResponse};
use serde_json::Value;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::groups::invalidate_cache;
use super::pack_assets;
use super::pack_embed;
use super::pack_format::{bad, parse_pack, parse_taxonomy};
use super::pack_import_steps::{
    audit, import_assets, import_embed, import_rules, import_taxonomy,
    import_theme,
};
use super::pack_store::{
    apply_taxonomy_guarded, apply_theme_restore, snapshot_to_payload,
};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

// ============ 导入（packs.rs 调用） ============

pub(super) async fn import(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    pack: &Value,
    confirm: bool,
) -> DomainResult<HttpResponse> {
    let head = parse_pack(pack)?;
    // 包体上限 1MiB。本地上传先被 `path_cfg::JSON_BODY_LIMIT`（2MiB，实测
    // 回「larger than allowed」）挡下，但商店远程路径不走 Json 提取器——
    // `fetch_remote_pack` 拉 16MiB 后直接进这里，所以闸必须建在共用入口上。
    if head.payload.to_string().len() > 1024 * 1024 {
        return Err(bad("包体 payload 超过 1MiB 上限"));
    }
    match head.kind.as_str() {
        "taxonomy" => import_taxonomy(state, actor, &head, confirm).await,
        "rules" => import_rules(state, actor, &head, confirm).await,
        "assets" => import_assets(state, actor, &head, confirm).await,
        "embed" => import_embed(state, actor, &head, confirm).await,
        _ => import_theme(state, actor, &head, confirm).await,
    }
}

pub(super) async fn rollback(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor: &i64,
    id: i64,
) -> DomainResult<HttpResponse> {
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    // FOR UPDATE：连点两次「回滚」时第二条必须等第一条删掉登记行，否则
    // 各自重放一次快照，下面的按序判据也一起失效
    let row: Option<(String, String, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT pack_id, kind, snapshot::text, applied_at \
         FROM content_packs WHERE id = $1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(internal)?;
    let Some((pack_id, kind, snapshot, applied_at)) = row else {
        return Err(DomainError::Validation("内容包记录不存在".into()));
    };
    // 后装先回：快照记的是「本次导入前」的状态，同 kind 若还有更晚的包，
    // 重放旧快照会把它们的覆盖一起吃掉，而它们的登记行仍显示「已装」
    // （批四十 P-147：回滚 A 再回滚 B，A 的内容还会借 B 的快照复活）
    let newer: Vec<String> = sqlx::query_scalar(
        "SELECT pack_id FROM content_packs WHERE kind = $1 AND id <> $2 \
         AND applied_at > $3 ORDER BY applied_at DESC LIMIT 3",
    )
    .bind(&kind)
    .bind(id)
    .bind(applied_at)
    .fetch_all(&mut *tx)
    .await
    .map_err(internal)?;
    if !newer.is_empty() {
        return Err(DomainError::Validation(format!(
            "请先回滚更晚安装的同类型包：{}",
            newer.join("、")
        )));
    }
    let snapshot: Value = serde_json::from_str(&snapshot)
        .map_err(|e| DomainError::Internal(e.into()))?;
    // R11：回滚需要知道「包写过哪些键」——从登记的 payload 读取
    // （0183 起随导入落表；旧登记行 payload 为 NULL 时按旧口径全删回内置）
    let pack: Value = sqlx::query_scalar::<_, Option<String>>(
        "SELECT payload::text FROM content_packs WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?
    .flatten()
    .and_then(|t| serde_json::from_str(&t).ok())
    .unwrap_or(Value::Null);
    if kind == "taxonomy" {
        // 快照 dict 带 sort；转回 payload 形状走同一落库路径
        let payload = snapshot_to_payload(&snapshot);
        let data = parse_taxonomy(&payload)?;
        apply_taxonomy_guarded(&mut tx, &data).await?;
    } else if kind == "theme" {
        let Some(settings) =
            snapshot.get("settings").and_then(Value::as_object)
        else {
            return Err(DomainError::Validation(
                "快照损坏（缺少 settings 节）".into(),
            ));
        };
        let (_, errors) = apply_theme_restore(&mut tx, settings).await?;
        if !errors.is_empty() {
            return Err(DomainError::FieldErrors(errors));
        }
    } else if kind == "rules" {
        // 快照 rules 只含导入前**已存在**的键；空快照 = 全部删除（回到内置默认）
        let snap_rules = snapshot
            .get("rules")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let cur_keys: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM site_settings WHERE name LIKE 'rule\\_%'",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(internal)?;
        // R11 修复：只删「本包导入写入的键」——导入后管理员手工新增的
        // rule_% 键不属于包，回滚不得误删。旧登记行（payload NULL）按旧口径。
        let packed_keys: Vec<String> = if pack.is_null() {
            cur_keys.clone()
        } else {
            pack.get("rules")
                .and_then(Value::as_object)
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default()
        };
        for key in cur_keys {
            if !snap_rules.contains_key(&key) && packed_keys.contains(&key) {
                sqlx::query("DELETE FROM site_settings WHERE name = $1")
                    .bind(&key)
                    .execute(&mut *tx)
                    .await
                    .map_err(internal)?;
            }
        }
        for (key, val) in &snap_rules {
            sqlx::query(
                "INSERT INTO site_settings (name, value) \
                 VALUES ($1, $2) ON CONFLICT (name) DO UPDATE \
                 SET value = EXCLUDED.value, updated_at = now()",
            )
            .bind(key)
            .bind(val.as_str().unwrap_or_default())
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        }
    } else if kind == "assets" {
        // 快照即 payload 形状，且只含包声明的那张表 → 重放范围 = 导入范围
        pack_assets::apply_assets(&mut tx, &snapshot).await?;
    } else if kind == "embed" {
        // 0190 起 embed 也登记 content_packs，但这里此前没有对应分支：
        // 装完就回滚不了，报「未知包类别」且登记行永远删不掉（P-146）
        pack_embed::rollback(&mut tx, &snapshot).await?;
    } else {
        return Err(DomainError::Validation("未知包类别".into()));
    }
    sqlx::query("DELETE FROM content_packs WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    audit(
        &mut tx,
        *actor,
        "content_pack:rollback",
        &serde_json::json!({ "pack_id": pack_id, "kind": kind }),
    )
    .await;
    tx.commit().await.map_err(internal)?;
    state.module_flags.invalidate().await;
    crate::cfgver::bump(state.get_ref(), "modules").await;
    invalidate_cache(state, "appearance").await;
    if kind == "embed" {
        crate::http::invalidate_home_cache(&**state).await;
    }
    Ok(ok(serde_json::json!({ "rolled_back": pack_id })))
}
