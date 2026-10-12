//! embed 包（0190）的入侧校验、落库、快照与回滚（批四十 P-142~P-148）。
//!
//! 为什么单独成文件：`pack_import.rs` / `pack_store.rs` 都在 300 行门禁的
//! 存量冻结清单里（只许瘦不许胖），而 embed 这条链要补的东西不少——原先
//! 它既没过后台保存侧的 `validate_body`，也没有回滚分支。
//!
//! 口径：
//! - **包路径不得比界面松**：首页公告的 iframe src 白名单
//!   （`http/home_news.rs`）直接读 `embed_origin`，一旦允许「模板落在
//!   embed_origin 前缀之外」，装个包就等于把嵌入源开到任意域；
//! - builtin 规则不可被包覆写（0176：内置种子跟随版本）。旧写法按 id
//!   `ON CONFLICT DO UPDATE` 并把 `builtin` 置 FALSE，下一次导入的
//!   DELETE 会把那几条内置规则一起删掉；
//! - 快照拍全 replay 写的十一列（B2/B3 同因：漏列 = 回滚把在用规则的
//!   展示名 / 比例 / 停用状态抹平）；
//! - 回滚 = 重放快照（同 assets 口径），走的还是这条 replay。

use serde_json::Value;
use sqlx::PgTransaction;

use crate::admin_http::embed_rule_check::{validate_body, EmbedRuleBody};
use crate::errors::{DomainError, DomainResult};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 快照行的列形状（与 `replay` 写入的列一一对应，缺列即回滚漏字段）
type SnapTuple = (
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    bool,
    i32,
);

/// 单行 → (id, 后台同款 body)。缺字段/形状不符按行报错——旧写法用
/// `unwrap_or_default()` 取串，「没写 url_pattern」会被铸成空规则入库。
fn row_of(r: &Value) -> Result<(i64, EmbedRuleBody), String> {
    let id = r.get("id").and_then(Value::as_i64).unwrap_or(0);
    if id <= 0 {
        return Err("缺少有效 id（需 >0，幂等与回滚全靠它）".into());
    }
    serde_json::from_value::<EmbedRuleBody>(r.clone())
        .map(|b| (id, b))
        .map_err(|e| format!("字段形状不符：{e}"))
}

/// 全量校验：后台 `validate_body` 六闸 + builtin 覆写拦截。预检与确认
/// 共用这一次调用，两态不可能口径分叉；错误按 FieldErrors 形状返回。
pub(super) async fn check_rows(
    tx: &mut PgTransaction<'_>,
    rows: &[Value],
) -> DomainResult<Vec<(i64, EmbedRuleBody)>> {
    let builtin: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM video_embed_rules WHERE builtin",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(internal)?;
    let mut errs: Vec<(String, String)> = Vec::new();
    let mut out: Vec<(i64, EmbedRuleBody)> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let key = format!("rules[{i}]");
        let (id, body) = match row_of(r) {
            Ok(v) => v,
            Err(m) => {
                errs.push((key, m));
                continue;
            }
        };
        if builtin.contains(&id) {
            errs.push((
                key,
                format!(
                    "id {id} 是内置规则：包覆写会把它降级成包项，下次导入即被删除"
                ),
            ));
            continue;
        }
        if let Err(e) = validate_body(&body) {
            errs.push((key, e.to_string()));
            continue;
        }
        out.push((id, body));
    }
    if !errs.is_empty() {
        return Err(DomainError::FieldErrors(errs));
    }
    Ok(out)
}

/// 纯覆盖落库：删包外非 builtin 行 → 逐行 upsert → 显式 id 不推进序列，
/// 补 setval（否则后台「新增规则」会撞上衣包写过的 id）。
pub(super) async fn replay(
    tx: &mut PgTransaction<'_>,
    rows: &[(i64, EmbedRuleBody)],
) -> DomainResult<usize> {
    let keep: Vec<i64> = rows.iter().map(|(id, _)| *id).collect();
    sqlx::query(
        "DELETE FROM video_embed_rules WHERE NOT builtin \
         AND NOT (id = ANY($1))",
    )
    .bind(&keep)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    let mut applied = 0usize;
    for (id, b) in rows {
        sqlx::query(
            "INSERT INTO video_embed_rules (id, provider, name_zh, \
             url_pattern, embed_template, embed_origin, render_kind, \
             aspect, extra_params, builtin, enabled, sort) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,FALSE,$10,$11) \
             ON CONFLICT (id) DO UPDATE SET \
             provider=EXCLUDED.provider, name_zh=EXCLUDED.name_zh, \
             url_pattern=EXCLUDED.url_pattern, \
             embed_template=EXCLUDED.embed_template, \
             embed_origin=EXCLUDED.embed_origin, \
             render_kind=EXCLUDED.render_kind, aspect=EXCLUDED.aspect, \
             extra_params=EXCLUDED.extra_params, \
             enabled=EXCLUDED.enabled, sort=EXCLUDED.sort",
        )
        .bind(id)
        .bind(b.provider.trim())
        .bind(b.name_zh.trim())
        .bind(&b.url_pattern)
        .bind(&b.embed_template)
        .bind(&b.embed_origin)
        .bind(&b.render_kind)
        .bind(&b.aspect)
        .bind(b.extra_params.as_deref())
        .bind(b.enabled)
        .bind(b.sort)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
        applied += 1;
    }
    sqlx::query(
        "SELECT setval('video_embed_rules_id_seq', \
         GREATEST((SELECT COALESCE(max(id),1) FROM video_embed_rules), 1))",
    )
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(applied)
}

/// 快照（回滚源）：非 builtin 行全量，十一列拍全
pub(super) async fn snapshot(
    tx: &mut PgTransaction<'_>,
) -> DomainResult<Value> {
    let rows: Vec<SnapTuple> = sqlx::query_as(
        "SELECT id, provider, name_zh, url_pattern, embed_template, \
         embed_origin, render_kind, aspect, extra_params, enabled, sort \
         FROM video_embed_rules WHERE NOT builtin ORDER BY id",
    )
    .fetch_all(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(serde_json::json!({
        "rules": rows.iter().map(|r| serde_json::json!({
            "id": r.0, "provider": r.1, "name_zh": r.2,
            "url_pattern": r.3, "embed_template": r.4,
            "embed_origin": r.5, "render_kind": r.6, "aspect": r.7,
            "extra_params": r.8, "enabled": r.9, "sort": r.10,
        })).collect::<Vec<_>>()
    }))
}

/// 快照 → bodies（宽松）：库内行必然满足 NOT NULL 形状，回滚不得因
/// 「旧版无校验时写进库的坏规则」而永久失败——那条规则本来就在库里，
/// 拦下回滚只会让它连带登记行一起卡死。
fn snapshot_bodies(rows: &[Value]) -> DomainResult<Vec<(i64, EmbedRuleBody)>> {
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let id = r.get("id").and_then(Value::as_i64).unwrap_or(0);
        if id <= 0 {
            return Err(DomainError::Validation(
                "快照损坏（行缺 id）".into(),
            ));
        }
        let body = serde_json::from_value::<EmbedRuleBody>(r.clone())
            .map_err(|e| {
                DomainError::Validation(format!("快照损坏（{e}）"))
            })?;
        out.push((id, body));
    }
    Ok(out)
}

/// 回滚：把快照按同一条 replay 重放（含被包覆写/删除的行）
pub(super) async fn rollback(
    tx: &mut PgTransaction<'_>,
    snapshot: &Value,
) -> DomainResult<usize> {
    let Some(rows) = snapshot.get("rules").and_then(Value::as_array) else {
        return Err(DomainError::Validation(
            "快照损坏（缺少 rules 节）".into(),
        ));
    };
    let bodies = snapshot_bodies(rows)?;
    replay(tx, &bodies).await
}
