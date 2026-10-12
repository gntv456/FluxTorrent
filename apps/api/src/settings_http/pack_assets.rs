//! 素材包（kind=assets）的入侧校验/消毒、快照与落库（批四十 P-141/142/143）。
//!
//! 从 `pack_store.rs` 按域拆出（那两个文件都在 300 行门禁的存量冻结清单里，
//! 只许瘦不许胖），并补上三条原先缺失的口径：
//! - **「包没声明的表不碰」**：内置素材种子每张只带一张表（0176），旧写法
//!   把缺键当成空集 → `DELETE ... WHERE NOT (id = ANY('{}'))` 清空另一张表，
//!   商店里点一次「安装」就蒸发全部勋章（或全部头像框）；
//! - **快照只拍包声明的表**：回滚范围 = 导入范围，多拍一张 = 回滚时连带
//!   吃掉那张表上的手工改动；
//! - **与后台道具 CRUD 同一道闸**：`medals` 的四条范围校验（价格/天数/
//!   加成/基点）与 `avatar_frames` 的两道净化（CSS 白名单、图片链接协议）
//!   此前只在界面侧存在，包路径一道没过。

use serde_json::Value;
use sqlx::PgTransaction;

use crate::errors::{DomainError, DomainResult};

fn internal(e: sqlx::Error) -> DomainError {
    DomainError::Internal(e.into())
}

/// 包输入过闸：界面怎么拦，包就怎么拦。返回归一化后的 payload（净化值入库）。
pub(super) fn check_input(
    payload: &Value,
) -> Result<Value, Vec<(String, String)>> {
    let tables = match payload.get("tables").and_then(Value::as_object) {
        Some(t) => t.clone(),
        None => return Ok(payload.clone()),
    };
    // medals：三条范围校验（0204 审计同口径，负价 = 买勋章倒赚钱）
    let medal_rows: Vec<Value> = tables
        .get("medals")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut errs: Vec<(String, String)> = Vec::new();
    for (i, r) in medal_rows.iter().enumerate() {
        for (k, lo, hi, msg) in [
            ("price", 0.0, f64::MAX, "价格不能为负"),
            ("duration_days", 0.0, f64::MAX, "有效天数不能为负"),
            ("bonus_addition_factor", 0.0, 10.0, "加成系数需在 0-10 之间"),
        ] {
            if let Some(v) = r.get(k).and_then(Value::as_f64) {
                if !(lo..=hi).contains(&v) {
                    errs.push((format!("medals[{i}]"), msg.into()));
                }
            }
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }
    // avatar_frames：两道净化（界面是过滤而非拒绝，包侧同）
    let mut out = payload.clone();
    let frames: Vec<Value> = tables
        .get("avatar_frames")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !frames.is_empty() {
        let clean: Vec<Value> = frames
            .iter()
            .map(|r| {
                let mut o = r.as_object().cloned().unwrap_or_default();
                let css = o
                    .get("css")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                o.insert(
                    "css".into(),
                    Value::String(crate::admin_p3_http::sanitize_frame_css(
                        css,
                    )),
                );
                let img = o
                    .get("image_url")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let kept = img
                    .as_deref()
                    .and_then(|v| {
                        crate::admin_p3_http::normalize_frame_image(Some(v))
                    });
                o.insert(
                    "image_url".into(),
                    kept.map(Value::String).unwrap_or(Value::Null),
                );
                Value::Object(o)
            })
            .collect();
        if let Some(t) = out
            .pointer_mut("/tables/avatar_frames")
            .and_then(Value::as_array_mut)
        {
            *t = clean;
        }
    }
    Ok(out)
}

/// 素材快照（回滚源）：只拍包声明的表，行形状与 `apply_assets` 读的键同名。
/// 行转 JSON 交给 SQL（两张表二十来列，Rust 侧元组要写四十行且漏一列就静默
/// 降级——B2/B3 的教训就是这么来的）。
pub(super) async fn snapshot_assets(
    tx: &mut PgTransaction<'_>,
    payload: &Value,
) -> DomainResult<Value> {
    let declared = payload
        .get("tables")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut out = serde_json::Map::new();
    if declared.contains_key("medals") {
        let rows: Value = sqlx::query_scalar(
            "SELECT COALESCE(json_agg(t ORDER BY t.id), '[]'::json) \
             FROM (SELECT id, name, price, rarity, description, asset_ref, \
                   duration_days, get_type, inventory, sale_begin_at, \
                   sale_end_at, \
                   bonus_addition_factor::float8 AS bonus_addition_factor, \
                   category_id, (limited::int <> 0) AS limited, \
                   gift_fee_bp, created_at \
                   FROM medals) t",
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(internal)?;
        out.insert("medals".into(), rows);
    }
    if declared.contains_key("avatar_frames") {
        let rows: Value = sqlx::query_scalar(
            "SELECT COALESCE(json_agg(t ORDER BY t.id), '[]'::json) \
             FROM (SELECT id, name, css, price, sort, image_url \
                   FROM avatar_frames) t",
        )
        .fetch_one(&mut **tx)
        .await
        .map_err(internal)?;
        out.insert("avatar_frames".into(), rows);
    }
    Ok(serde_json::json!({ "tables": Value::Object(out) }))
}

/// 素材纯覆盖落库：**只重建包声明的表**；包内行按 id upsert，
/// 包外表行删除——被引用的跳过（medals 被 user_medals 持有、frames
/// 被用户佩戴，宁残留不破坏）。
pub(super) async fn apply_assets(
    tx: &mut PgTransaction<'_>,
    payload: &Value,
) -> DomainResult<(usize, usize)> {
    let Some(tables) = payload.get("tables").and_then(Value::as_object) else {
        return Err(DomainError::Validation(
            "assets 包缺少 payload.tables".into(),
        ));
    };
    for t in tables.keys() {
        if !super::pack_format::ASSET_TABLES.contains(&t.as_str()) {
            return Err(DomainError::Validation(format!(
                "assets 包不允许触碰表：{t}"
            )));
        }
    }
    // ---- medals ----
    let medal_rows: Vec<Value> = tables
        .get("medals")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let medal_ids: Vec<i64> = medal_rows
        .iter()
        .filter_map(|r| r.get("id").and_then(Value::as_i64))
        .collect();
    if tables.contains_key("medals") {
        sqlx::query(
            "DELETE FROM medals WHERE NOT (id = ANY($1)) AND NOT EXISTS \
             (SELECT 1 FROM user_medals um WHERE um.medal_id = medals.id)",
        )
        .bind(&medal_ids)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    }
    let mut n_medals = 0usize;
    for r in &medal_rows {
        let (Some(id), Some(name)) = (
            r.get("id").and_then(Value::as_i64),
            r.get("name").and_then(Value::as_str),
        ) else {
            continue;
        };
        let dt = |k: &str| -> Option<String> {
            r.get(k).and_then(Value::as_str).map(str::to_string)
        };
        sqlx::query(
            "INSERT INTO medals (id, name, price, rarity, description, \
             asset_ref, duration_days, get_type, inventory, sale_begin_at, \
             sale_end_at, bonus_addition_factor, category_id, limited, \
             gift_fee_bp, created_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, \
             $10::timestamptz, $11::timestamptz, $12, $13, $14, $15, \
             COALESCE($16::timestamptz, now())) \
             ON CONFLICT (id) DO UPDATE SET \
             name = EXCLUDED.name, price = EXCLUDED.price, \
             rarity = EXCLUDED.rarity, description = EXCLUDED.description, \
             asset_ref = EXCLUDED.asset_ref, \
             duration_days = EXCLUDED.duration_days, \
             get_type = EXCLUDED.get_type, inventory = EXCLUDED.inventory, \
             sale_begin_at = EXCLUDED.sale_begin_at, \
             sale_end_at = EXCLUDED.sale_end_at, \
             bonus_addition_factor = EXCLUDED.bonus_addition_factor, \
             category_id = EXCLUDED.category_id, limited = EXCLUDED.limited, \
             gift_fee_bp = COALESCE(EXCLUDED.gift_fee_bp, medals.gift_fee_bp)",
        )
        .bind(id)
        .bind(name)
        .bind(r.get("price").and_then(Value::as_i64))
        .bind(r.get("rarity").and_then(Value::as_str))
        .bind(r.get("description").and_then(Value::as_str))
        .bind(r.get("asset_ref").and_then(Value::as_str))
        .bind(
            r.get("duration_days")
                .and_then(Value::as_i64)
                .map(|v| v as i32),
        )
        .bind(r.get("get_type").and_then(Value::as_i64).unwrap_or(1) as i16)
        .bind(r.get("inventory").and_then(Value::as_i64).map(|v| v as i32))
        .bind(dt("sale_begin_at"))
        .bind(dt("sale_end_at"))
        .bind(
            r.get("bonus_addition_factor")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
        )
        .bind(r.get("category_id").and_then(Value::as_i64).unwrap_or(0) as i32)
        .bind(r.get("limited").and_then(Value::as_bool).unwrap_or(false))
        .bind(r.get("gift_fee_bp").and_then(Value::as_i64).map(|v| v as i32))
        .bind(dt("created_at"))
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
        n_medals += 1;
    }
    // ---- avatar_frames ----
    let frame_rows: Vec<Value> = tables
        .get("avatar_frames")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let frame_ids: Vec<i32> = frame_rows
        .iter()
        .filter_map(|r| r.get("id").and_then(Value::as_i64).map(|v| v as i32))
        .collect();
    if tables.contains_key("avatar_frames") {
        sqlx::query(
            "DELETE FROM avatar_frames WHERE NOT (id = ANY($1)) \
             AND NOT EXISTS (\
               SELECT 1 FROM users u WHERE u.avatar_frame_id = \
               avatar_frames.id)",
        )
        .bind(&frame_ids)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    }
    let mut n_frames = 0usize;
    for r in &frame_rows {
        let (Some(id), Some(name), Some(css)) = (
            r.get("id").and_then(Value::as_i64).map(|v| v as i32),
            r.get("name").and_then(Value::as_str),
            r.get("css").and_then(Value::as_str),
        ) else {
            continue;
        };
        sqlx::query(
            "INSERT INTO avatar_frames (id, name, css, price, sort, \
             image_url) VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, \
             css = EXCLUDED.css, price = EXCLUDED.price, \
             sort = EXCLUDED.sort, image_url = EXCLUDED.image_url",
        )
        .bind(id)
        .bind(name)
        .bind(css)
        .bind(r.get("price").and_then(Value::as_i64).unwrap_or(0) as i32)
        .bind(r.get("sort").and_then(Value::as_i64).unwrap_or(0) as i32)
        .bind(r.get("image_url").and_then(Value::as_str))
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
        n_frames += 1;
    }
    Ok((n_medals, n_frames))
}
