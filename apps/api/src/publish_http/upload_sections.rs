//! 发种多维属性与标签入库（M04 第八轮 Section）。
//! 从 publish_http/upload.rs 按域拆出；校验 kind 白名单后写 torrent_sections + tags。
//!
//! B 批承重墙·B2（2026-09-25）：内容侧维度从「只能单选枚举」升级为**六类型字段系统**
//! （0195 数据模型 + `crate::fields` 共享校验器），与用户侧自定义字段同一套语义。
//!
//! 协议兼容：值可以是
//!   · JSON number  ⇒ 旧格式，枚举单选（存量客户端零改动）
//!   · JSON object  ⇒ 新格式，按 field_type 表达：
//!       {"dict_ids":[5,8]} / {"text":"…"} / {"number":320}
//!       / {"date":"2024-05-01"} / {"bool":true}
//! 判据在**值的 JSON 类型**上，不靠参数名 ⇒ 兼容面最广。

use actix_web::web;
use serde_json::{Map, Value};

use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;
use crate::state::AppState;

use super::ptgen::UploadForm;

/// 某维度的定义（0195 扩列后）。
#[derive(sqlx::FromRow)]
struct KindDef {
    kind: String,
    label: String,
    field_type: String,
    required: bool,
    multiple: bool,
    #[allow(dead_code)]
    enabled: bool,
}

/// 解析后的单个维度取值。
///
/// 两种形态并存：`dict_ids` = 枚举（写 `dict_id` 列）；`values` = 自由值
/// （写 `value` 列）。同一维度的多值按 `ordinal` 递增落库。
pub(crate) struct SectionValue {
    pub kind: String,
    /// 枚举值：`section_dict.id`，按出现顺序
    pub dict_ids: Vec<i64>,
    /// 自由值：按出现顺序（text/number/date/bool 各一个）
    pub values: Vec<Value>,
}

impl SectionValue {
    /// 该维度是否完全无值（用于必填判定）。
    fn is_empty(&self) -> bool {
        self.dict_ids.is_empty() && self.values.is_empty()
    }
}

/// 读取全部启用中的维度定义。
async fn load_kinds(db: &sqlx::PgPool) -> DomainResult<Vec<KindDef>> {
    sqlx::query_as(
        "SELECT kind, label, field_type, required, multiple, enabled \
         FROM section_kinds WHERE enabled ORDER BY sort, kind",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// 解析并**整体**校验 sections：任一维度/选项非法就一条都不写。
///
/// 发种主链必须在 `INSERT torrents` 之前先调它。原先校验与写入交织在同一个循环里，
/// 后一个维度报错时种子已经入库、前面的归属也已落库——用户只看到一个 400，
/// 而重试同一个 .torrent 会永远撞 TorrentDuplicate。
///
/// B2 起支持六类型与多值；校验规则：
///   1. kind 必须在 `section_kinds` 且 `enabled`
///   2. 值形状匹配该 kind 的 `field_type`（走 `crate::fields` 共享校验器）
///   3. `multiple=false` 的维度只允许一个值
///   4. `required=true` 的维度必须给非空值
///   5. 枚举值必须属于该 kind 的 `section_dict`
///
/// `strict` = **编辑语义**：`true` 时要求把 enabled 维度全部给全（缺 = 视为清空，
/// 但仍受 `required` 约束）——编辑表单是整表单提交，缺项不能算「不动」。
/// 发种入口传 `false`（只校验给定项 + 必填）。
pub(crate) async fn parse_sections(
    db: &sqlx::PgPool,
    raw: Option<&String>,
) -> DomainResult<Vec<SectionValue>> {
    parse_sections_ex(db, raw, false).await
}

/// `parse_sections` 的可配版（`strict` 见上）。编辑口与批量口用 `true`。
pub(crate) async fn parse_sections_ex(
    db: &sqlx::PgPool,
    raw: Option<&String>,
    strict: bool,
) -> DomainResult<Vec<SectionValue>> {
    let _ = strict;
    let kinds = load_kinds(db).await?;
    let json = match raw.map(|s| s.as_str().trim()).filter(|s| !s.is_empty()) {
        Some(j) => j,
        None => {
            // 没传 sections：仍要挡必填维度
            check_required(&kinds, &[])?;
            return Ok(Vec::new());
        }
    };
    let map: Map<String, Value> = serde_json::from_str(json)
        .map_err(|_| DomainError::Validation("sections 需为 JSON 对象".into()))?;

    let mut out: Vec<SectionValue> = Vec::new();
    for (kind, raw_val) in &map {
        let Some(def) = kinds.iter().find(|k| &k.kind == kind) else {
            return Err(DomainError::Validation(format!("未知维度 {kind}")));
        };
        let sv = parse_one(db, def, raw_val).await?;
        if !sv.is_empty() {
            out.push(sv);
        }
    }
    // 必填维度缺值 ⇒ 400（新增能力：四审 L3 指出内容侧原无必填概念）
    check_required(&kinds, &out)?;
    Ok(out)
}

/// **写**已解析的维度取值（编辑口/发种口共用）。
///
/// 语义：整组重建。`parsed` 里出现的维度先清后写；**调用方决定未出现的维度**
/// 是保留还是清空——`clear_absent=true` 时把 `parsed` 未含的维度也清掉
/// （编辑表单「整表单保存」口径），`false` 则只动给定的维度（发种口径）。
pub(crate) async fn write_sections(
    db: &sqlx::PgPool,
    torrent_id: i64,
    parsed: &[SectionValue],
    clear_absent: bool,
) -> DomainResult<()> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if clear_absent {
        // 只清「本次没提交」的维度；已提交的维度下面逐个重建
        let handled: Vec<&str> = parsed.iter().map(|s| s.kind.as_str()).collect();
        sqlx::query(
            "DELETE FROM torrent_sections WHERE torrent_id = $1 \
             AND NOT (kind = ANY($2))",
        )
        .bind(torrent_id)
        .bind(&handled)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    for sv in parsed {
        write_one(&mut tx, torrent_id, sv).await?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 写单个维度：先清该维度旧行，再按 `ordinal` 顺序重插。
async fn write_one(
    tx: &mut sqlx::PgConnection,
    torrent_id: i64,
    sv: &SectionValue,
) -> DomainResult<()> {
    sqlx::query("DELETE FROM torrent_sections WHERE torrent_id = $1 AND kind = $2")
        .bind(torrent_id)
        .bind(&sv.kind)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut ord: i32 = 0;
    for dict_id in &sv.dict_ids {
        sqlx::query(
            "INSERT INTO torrent_sections (torrent_id, kind, dict_id, ordinal) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(torrent_id)
        .bind(&sv.kind)
        .bind(dict_id)
        .bind(ord)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        ord += 1;
    }
    for v in &sv.values {
        sqlx::query(
            "INSERT INTO torrent_sections (torrent_id, kind, value, ordinal) \
             VALUES ($1, $2, $3, $4)",
        )
        .bind(torrent_id)
        .bind(&sv.kind)
        .bind(v)
        .bind(ord)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        ord += 1;
    }
    Ok(())
}

/// 必填维度缺值判定（错误文案带显示名）。
fn check_required(kinds: &[KindDef], got: &[SectionValue]) -> DomainResult<()> {
    let missing: Vec<&str> = kinds
        .iter()
        .filter(|k| k.required)
        .filter(|k| !got.iter().any(|s| s.kind == k.kind))
        .map(|k| k.label.as_str())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(DomainError::Validation(format!(
        "必填维度未填写：{}",
        missing.join("、")
    )))
}

/// 解析单个维度取值（新/旧格式分派 + 类型校验 + 归属校验）。
async fn parse_one(
    db: &sqlx::PgPool,
    def: &KindDef,
    raw_val: &Value,
) -> DomainResult<SectionValue> {
    let mut dict_ids: Vec<i64> = Vec::new();
    let mut values: Vec<Value> = Vec::new();

    match raw_val {
        // ---- 旧格式：JSON number = 枚举单选 ----
        Value::Number(_) => {
            let id = raw_val.as_i64().ok_or_else(|| {
                DomainError::Validation(format!("维度 {} 的值非法", def.kind))
            })?;
            dict_ids.push(id);
        }
        Value::Null => {}
        // ---- 新格式：JSON object ----
        Value::Object(o) => {
            // 枚举：{"dict_ids":[...]}
            if let Some(arr) = o.get("dict_ids") {
                let arr = arr.as_array().ok_or_else(|| {
                    DomainError::Validation(format!(
                        "维度「{}」的 dict_ids 需为数组",
                        def.label
                    ))
                })?;
                for item in arr {
                    let id = item.as_i64().ok_or_else(|| {
                        DomainError::Validation(format!(
                            "维度「{}」的 dict_ids 元素需为整数",
                            def.label
                        ))
                    })?;
                    if !dict_ids.contains(&id) {
                        dict_ids.push(id);
                    }
                }
            }
            // 自由值：按 field_type 取键；枚举维度只认 dict_ids
            let free_key = match def.field_type.as_str() {
                "text" => Some("text"),
                "number" => Some("number"),
                "date" => Some("date"),
                "bool" => Some("bool"),
                _ => None,
            };
            let candidate = free_key
                .and_then(|k| o.get(k))
                // 兼容写法：{"value": ...} 按 field_type 解释
                .or_else(|| o.get("value"));
            if let Some(v) = candidate {
                crate::fields::validate_value(
                    &def.field_type,
                    &serde_json::json!([]),
                    v,
                )
                .map_err(|e| {
                    DomainError::Validation(format!("维度「{}」：{e}", def.label))
                })?;
                if !crate::fields::is_empty_value(v) {
                    values.push(v.clone());
                }
            }
        }
        _ => {
            return Err(DomainError::Validation(format!(
                "维度「{}」的值格式非法（需为整数或对象）",
                def.label
            )))
        }
    }

    // 多值约束：非 multiple 维度只允许单值
    if !def.multiple && dict_ids.len() + values.len() > 1 {
        return Err(DomainError::Validation(format!(
            "维度「{}」不支持多值（请在维度设置里开启多值）",
            def.label
        )));
    }

    // 枚举值归属校验：外键只保证字典行存在，不保证没挂错维度
    for id in &dict_ids {
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM section_dict WHERE id = $1 AND kind = $2)",
        )
        .bind(id)
        .bind(&def.kind)
        .fetch_one(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if !ok {
            return Err(DomainError::Validation(format!(
                "维度「{}」的字典项 {} 不存在",
                def.label, id
            )));
        }
    }

    Ok(SectionValue {
        kind: def.kind.clone(),
        dict_ids,
        values,
    })
}

pub(super) async fn store_sections_tags(
    state: &web::Data<std::sync::Arc<AppState>>,
    form: &UploadForm,
    auth: &AuthUser,
    id: i64,
) -> DomainResult<()> {
    // 多维属性（B2 六类型）：parse_sections 已整体校验，这里只写
    let parsed = parse_sections(&state.repo.db, form.sections.as_ref()).await?;
    if !parsed.is_empty() {
        // 发种口径：只写给定维度，未出现的维度本就没行可清
        write_sections(&state.repo.db, id, &parsed, false).await?;
        // 反向落旧三列：只写 sections 的种子也要能被「按媒介/学段/版本」筛到
        crate::torrents::sync_legacy_columns(&state.repo.db, id).await?;
    }

    // 标签（NP upload.php tags 口径）：发布时直接打标；统一走 apply_torrent_tags
    // （0159：校验 + official_tag 联动三入口同源，官种物化列不再漂移）
    if let Some(json) = form.tags.as_deref().map(str::trim).filter(|j| !j.is_empty())
    {
        let ids: Vec<i32> = serde_json::from_str(json)
            .map_err(|_| DomainError::Validation("tags 需为 JSON 数组".into()))?;
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
