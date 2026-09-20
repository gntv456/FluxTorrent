//! GET /admin/settings/schema：分组结构 + 字段声明 + 当前值 + 权限掩码。
//! 从 settings_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, web, HttpRequest, HttpResponse};

use super::meta::{group_label, group_order, role_of, write_min};
use super::meta::{MetaRow, META_SELECT, SETTINGS_WRITE_MIN};

// GET /admin/settings/schema
// ============================================================================

#[derive(serde::Serialize)]
struct FieldOut {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    label: String,
    label_en: Option<String>,
    hint: Option<String>,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    options: Option<serde_json::Value>,
    secret: bool,
    readonly: bool,
    writable: bool,
    /// 密文字段恒为空串（只给「已设置」状态），其余为当前值
    value: String,
    configured: bool,
    updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize)]
struct CardOut {
    key: String,
    fields: Vec<FieldOut>,
}

#[derive(serde::Serialize)]
struct GroupOut {
    key: String,
    label: String,
    writable: bool,
    count: usize,
    cards: Vec<CardOut>,
}

/// 保持 SQL 排序的分区 / 卡片 / 字段三层结构
type Grouped = Vec<(String, Vec<(String, Vec<FieldOut>)>)>;

fn push_field(grouped: &mut Grouped, grp: &str, card: &str, field: FieldOut) {
    let gi = match grouped.iter().position(|(g, _)| g == grp) {
        Some(i) => i,
        None => {
            grouped.push((grp.to_string(), Vec::new()));
            grouped.len() - 1
        }
    };
    let cards = &mut grouped[gi].1;
    let ci = match cards.iter().position(|(c, _)| c == card) {
        Some(i) => i,
        None => {
            cards.push((card.to_string(), Vec::new()));
            cards.len() - 1
        }
    };
    cards[ci].1.push(field);
}

#[get("/admin/settings/schema")]
pub(super) async fn settings_schema(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let order = group_order(&state).await;
    // SQL 已按 (grp, group_key, card_order, name) 排序 → 保序填充即得正确卡片/字段顺序
    let rows: Vec<MetaRow> = sqlx::query_as(&format!(
        "{META_SELECT} WHERE COALESCE(m.visible, true) \
         ORDER BY COALESCE(s.grp, 'misc'), COALESCE(m.group_key, ''), \
                  COALESCE(m.card_order, 9000), s.name"
    ))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut grouped: Grouped = Vec::new();
    let mut field_count = 0usize;
    for m in &rows {
        let field = FieldOut {
            name: m.name.clone(),
            kind: m.kind.clone(),
            label: m
                .label_zh
                .clone()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| m.label_en.clone())
                .unwrap_or_else(|| m.name.clone()),
            label_en: m.label_en.clone(),
            hint: m.hint.clone(),
            unit: m.unit.clone(),
            min: m.min,
            max: m.max,
            step: m.step,
            options: m.options.clone(),
            secret: m.secret,
            readonly: m.readonly,
            writable: !m.readonly && auth.class_id >= write_min(m),
            value: if m.secret {
                String::new()
            } else {
                m.value.clone()
            },
            configured: !m.secret || !m.value.is_empty(),
            updated_at: m.updated_at,
        };
        let card = m.group_key.clone().unwrap_or_else(|| "默认".to_string());
        push_field(&mut grouped, &m.grp, &card, field);
        field_count += 1;
    }

    // 分区顺序：settings_group_order 优先，其余按发现顺序追加
    let mut ordered: Vec<String> = order
        .iter()
        .filter(|g| grouped.iter().any(|(k, _)| k == *g))
        .cloned()
        .collect();
    for (g, _) in &grouped {
        if !ordered.contains(g) {
            ordered.push(g.clone());
        }
    }

    let mut groups: Vec<GroupOut> = Vec::new();
    for g in ordered {
        let cards_vec = grouped
            .iter()
            .position(|(k, _)| *k == g)
            .map(|i| grouped.remove(i).1)
            .unwrap_or_default();
        let count: usize = cards_vec.iter().map(|(_, f)| f.len()).sum();
        let writable =
            cards_vec.iter().any(|(_, f)| f.iter().any(|x| x.writable));
        let cards: Vec<CardOut> = cards_vec
            .into_iter()
            .map(|(key, fields)| CardOut { key, fields })
            .collect();
        groups.push(GroupOut {
            key: g.clone(),
            label: group_label(&g).to_string(),
            writable,
            count,
            cards,
        });
    }

    Ok(ok(serde_json::json!({
        "role": role_of(auth.class_id),
        "editable": auth.class_id >= SETTINGS_WRITE_MIN,
        "field_count": field_count,
        "group_count": groups.len(),
        "groups": groups,
    })))
}

// ============================================================================
