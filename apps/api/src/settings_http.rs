//! 站点设定（hxpt / NexusPHP settings.php 复刻）—— 方案 P0 后端。
//!
//! 与既有的 `GET/PUT /admin/settings`（admin_http.rs，键值对原型）并存，本模块提供
//! 「声明 + 分组批量 + 校验 + 审计 + 缓存失效」协议：
//!
//! | 接口 | 说明 |
//! | :--- | :--- |
//! | `GET  /admin/settings/schema`   | 分组结构 + 字段声明（类型/单位/范围/枚举/密文/只读）+ 当前值 + 权限掩码 |
//! | `PUT  /admin/settings/groups`   | 按分区批量保存；逐字段校验（任一失败整体回滚并返回字段级错误） |
//! | `POST /admin/settings/validate` | 单字段预校验（前端失焦即时反馈） |
//! | `GET  /admin/settings/history`  | 单项修改历史（来自 audit_log 的 setting:update） |
//!
//! 权威校验一律在后端执行（§4.2）：type / range / enum / url / required。
//! secret 字段 GET 只回掩码，保存时留空 = 保持不变，审计不记明文。
//! 保存成功即失效 `settings:{grp}` 缓存并广播 `settings:changed`（§7.2）。

use actix_web::{get, post, put, web, HttpRequest, HttpResponse};
use redis::AsyncCommands;
use serde::Deserialize;
use std::collections::HashMap;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 站点设定默认可写门槛（sysop；字段级可经 settings_meta.min_class 放宽）
const SETTINGS_WRITE_MIN: i32 = 99;
/// 单次批量保存的字段上限
const MAX_BATCH: usize = 500;

pub fn mount_settings(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(settings_schema)
        .service(settings_groups_put)
        .service(settings_validate)
        .service(settings_history)
        .service(settings_export)
        .service(settings_import)
}

// ============================================================================
// 元数据行（settings_meta ⋈ site_settings）
// ============================================================================

#[derive(sqlx::FromRow, Clone)]
struct MetaRow {
    name: String,
    kind: String,
    label_zh: Option<String>,
    label_en: Option<String>,
    hint: Option<String>,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    options: Option<serde_json::Value>,
    secret: bool,
    readonly: bool,
    group_key: Option<String>,
    min_class: Option<i32>,
    grp: String,
    value: String,
    updated_at: chrono::DateTime<chrono::Utc>,
}

const META_SELECT: &str = "SELECT s.name, m.type AS kind, m.label_zh, m.label_en, m.hint, m.unit, \
        m.min, m.max, m.step, m.options, \
        COALESCE(m.secret, false) AS secret, COALESCE(m.readonly, false) AS readonly, \
        m.group_key, m.min_class, \
        COALESCE(s.grp, 'misc') AS grp, s.value, s.updated_at \
     FROM site_settings s JOIN settings_meta m ON m.name = s.name";

/// 字段可写门槛（meta.min_class 缺省 = sysop）
fn write_min(m: &MetaRow) -> i32 {
    m.min_class.unwrap_or(SETTINGS_WRITE_MIN)
}

fn role_of(class_id: i32) -> &'static str {
    if class_id >= 99 {
        "sysop"
    } else if class_id >= 93 {
        "administrator"
    } else {
        "moderator"
    }
}

/// 12 分区中文名（前端可再用 i18n 字典覆盖）
fn group_label<'a>(key: &'a str) -> &'a str {
    match key {
        "basic" => "基础设定",
        "main" => "主要设定",
        "smtp" => "SMTP 设定",
        "security" => "安全设定",
        "authority" => "权限设定",
        "tweak" => "次要设定",
        "bonus" => "魔力设定",
        "account" => "账号设定",
        "torrent" => "种子设定",
        "attachment" => "附件设定",
        "advertisement" => "广告设定",
        "misc" => "其他设定",
        // U1/U5 扩展分区（0107/0109/0112 播种；未列入 DEFAULT_GROUP_ORDER 时排在末尾）
        "economy" => "经济参数",
        "anticheat" => "防作弊",
        "module_community" => "模块开关·社区",
        "module_economy" => "模块开关·经济",
        "module_fun" => "模块开关·娱乐",
        "module_ops" => "模块开关·运营",
        other => other,
    }
}

const DEFAULT_GROUP_ORDER: &[&str] = &[
    "basic",
    "main",
    "smtp",
    "security",
    "authority",
    "tweak",
    "bonus",
    "account",
    "torrent",
    "attachment",
    "advertisement",
    "anticheat",
    "economy",
    "module_community",
    "module_economy",
    "module_fun",
    "module_ops",
    "misc",
];

/// 分区顺序来自元数据键 settings_group_order；缺失则回退内置顺序
async fn group_order(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> Vec<String> {
    let raw: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'settings_group_order'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    let parsed: Vec<String> = raw
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if parsed.is_empty() {
        DEFAULT_GROUP_ORDER.iter().map(|s| s.to_string()).collect()
    } else {
        parsed
    }
}

// ============================================================================
// 校验引擎（§4.2：type / range / enum / url / required）
// ============================================================================

fn is_hex_color(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(|c| c.is_ascii_hexdigit())
}

/// URL / 主机地址校验：容忍无协议的 host:port（如 127.0.0.1:3000），拒绝空白与无点主机
fn looks_like_url(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.contains(char::is_whitespace) {
        return false;
    }
    let rest = t
        .strip_prefix("https://")
        .or_else(|| t.strip_prefix("http://"))
        .unwrap_or(t);
    let host = rest.split('/').next().unwrap_or("");
    let host_no_port = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    !host_no_port.is_empty()
        && (host_no_port.contains('.') || host_no_port == "localhost")
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// 校验并归一化单个字段值；`Ok(值)` / `Err(错误说明)`
fn validate_field(m: &MetaRow, raw: &str) -> Result<String, String> {
    let v = raw.trim().to_string();
    let rule = m
        .options
        .as_ref()
        .and_then(|o| o.get("rule"))
        .and_then(|r| r.as_str())
        .unwrap_or("");
    let required = m
        .options
        .as_ref()
        .and_then(|o| o.get("required"))
        .and_then(|r| r.as_bool())
        .unwrap_or(false);

    match m.kind.as_str() {
        "number" => {
            if v.is_empty() {
                return Err("不能为空".into());
            }
            let n: f64 = v.parse().map_err(|_| "必须是数字".to_string())?;
            if let Some(min) = m.min {
                if n < min {
                    return Err(format!("不得小于 {}", fmt_num(min)));
                }
            }
            if let Some(max) = m.max {
                if n > max {
                    return Err(format!("不得大于 {}", fmt_num(max)));
                }
            }
            if m.step.unwrap_or(1.0) == 1.0 && n.fract() != 0.0 {
                return Err("必须是整数".into());
            }
        }
        "yesno" => {
            if v != "yes" && v != "no" {
                return Err("只能取 yes / no".into());
            }
        }
        "enum" => {
            let allowed: Vec<String> = m
                .options
                .as_ref()
                .and_then(|o| o.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            x.get("v")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                        })
                        .collect()
                })
                .unwrap_or_default();
            if !allowed.is_empty() && !allowed.iter().any(|x| x == &v) {
                return Err(format!("取值须为 {}", allowed.join(" / ")));
            }
        }
        "color" => {
            if !is_hex_color(&v) {
                return Err("须为合法 hex 颜色，如 #FFD700".into());
            }
        }
        "classlevel" => {
            if v.is_empty() {
                return Err("不能为空".into());
            }
            let n: i32 = v.parse().map_err(|_| "必须是等级数字".to_string())?;
            if !(0..=100).contains(&n) {
                return Err("等级取值 0-100".into());
            }
        }
        "password" => {
            if raw.len() > 4096 {
                return Err("长度不得超过 4096".into());
            }
        }
        "textarea" => {
            if raw.len() > 65535 {
                return Err("长度不得超过 65535".into());
            }
        }
        // text / pair
        _ => {
            if required && v.is_empty() {
                return Err("不能为空".into());
            }
            if v.len() > 4096 {
                return Err("长度不得超过 4096".into());
            }
            if !v.is_empty() {
                match rule {
                    "url" => {
                        if !looks_like_url(&v) {
                            return Err("须为合法 URL 或 host:port".into());
                        }
                    }
                    "csv_ids" => {
                        if !v
                            .split(',')
                            .all(|p| p.trim().parse::<i64>().is_ok())
                        {
                            return Err("须为逗号分隔的数字 ID".into());
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // 密文保留原始字节（避免误 trim 密码首尾空格），其余统一 trim
    Ok(if m.kind == "password" {
        raw.to_string()
    } else {
        v
    })
}

// ============================================================================
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
async fn settings_schema(
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
// POST /admin/settings/validate —— 单字段预校验
// ============================================================================

#[derive(Deserialize)]
struct ValidateReq {
    name: String,
    #[serde(default)]
    value: String,
}

#[post("/admin/settings/validate")]
async fn settings_validate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ValidateReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let name = body.name.trim();
    let meta: Option<MetaRow> =
        sqlx::query_as(&format!("{META_SELECT} WHERE s.name = $1"))
            .bind(name)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let meta = meta
        .ok_or_else(|| DomainError::Validation(format!("未知设定项 {name}")))?;
    match validate_field(&meta, &body.value) {
        Ok(normalized) => Ok(ok(serde_json::json!({
            "valid": true, "name": name, "normalized": normalized,
        }))),
        Err(e) => Err(DomainError::FieldErrors(vec![(name.to_string(), e)])),
    }
}

// ============================================================================
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

fn push_unique(out: &mut Vec<String>, s: &str) {
    if !out.iter().any(|x| x == s) {
        out.push(s.to_string());
    }
}

/// 关键字段变更后的影响提示（§4.2 effects）
fn effects_for(names: &[String]) -> Vec<String> {
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
async fn invalidate_cache(
    state: &web::Data<std::sync::Arc<AppState>>,
    group: &str,
) {
    let mut conn = state.redis.clone();
    let _: () = conn.del(format!("settings:{group}")).await.unwrap_or(());
    let _: () = conn.del("settings:all").await.unwrap_or(());
    // 模块开关（module_*）属 module 组：本进程缓存立即失效（U1 §5.1，最坏竞态由 TTL 兜底）
    state.module_flags.invalidate().await;
    let _: i64 = conn.publish("settings:changed", group).await.unwrap_or(0);
}

#[put("/admin/settings/groups")]
async fn settings_groups_put(
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
        sqlx::query("UPDATE site_settings SET value = $2, updated_at = now() WHERE name = $1")
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

fn truncate(s: &str) -> String {
    const MAX: usize = 500;
    if s.chars().count() <= MAX {
        s.to_string()
    } else {
        let head: String = s.chars().take(MAX).collect();
        format!("{head}…")
    }
}

// ============================================================================
// GET /admin/settings/history —— 单项修改历史（audit_log）
// ============================================================================

#[derive(Deserialize)]
struct HistoryQ {
    name: String,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

fn default_page() -> i64 {
    1
}

fn default_per_page() -> i64 {
    20
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct HistoryRow {
    id: i64,
    action: String,
    actor: Option<String>,
    actor_id: Option<i64>,
    detail: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/settings/history")]
async fn settings_history(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<HistoryQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let per_page = q.per_page.clamp(1, 100);
    let name = q.name.trim();
    let rows: Vec<HistoryRow> = sqlx::query_as(
        "SELECT a.id, a.action, u.username AS actor, a.actor_id, a.ref AS detail, a.created_at \
         FROM audit_log a LEFT JOIN users u ON u.id = a.actor_id \
         WHERE a.action = 'setting:update' AND a.ref->>'setting' = $1 \
         ORDER BY a.id DESC LIMIT $2 OFFSET $3",
    )
    .bind(name)
    .bind(per_page)
    .bind((q.page.max(1) - 1) * per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log WHERE action = 'setting:update' AND ref->>'setting' = $1",
    )
    .bind(name)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "name": name,
        "rows": rows,
        "total": total,
        "page": q.page.max(1),
        "per_page": per_page,
    })))
}

// ============================================================================
// 导出 / 导入（P2 §4.3）
// ============================================================================
//
// 导出：全量配置 JSON。密文字段默认掩码（值为空串并列入 masked_secrets），
//       `plaintext=1` 时以明文导出（仅 sysop）。
// 导入：仅 sysop。首次调用（confirm 缺省）为空跑，只回字段级 diff；带
//       `confirm=true` 才落库。逐字段复用同一套 validate_field。
//       密文空值语义随 `plaintext` 开关而定：
//         - plaintext=false（默认）：空值 = 保持不变，与「导出默认掩码」一致，
//           避免把掩码快照里的空串误当成"清空密钥"。
//         - plaintext=true：空值 = 按字面还原为空，保证
//           「plaintext 导出 → 清库 → 导入 → 再导出」严格 diff 一致。
//       导出快照自带 `plaintext` 字段，前端解析后原样回传即可。

#[derive(Deserialize)]
struct ExportQ {
    /// 1 = 密文字段以明文导出（仅 sysop）
    #[serde(default)]
    plaintext: u8,
}

#[get("/admin/settings/export")]
async fn settings_export(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ExportQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let plaintext = q.plaintext == 1;
    if plaintext {
        crate::authz::require_perm(
            &state,
            &auth,
            crate::authz::perm::SETTINGS_MANAGE,
        )
        .await?;
    }
    let rows: Vec<MetaRow> = sqlx::query_as(&format!(
        "{META_SELECT} ORDER BY s.grp, m.group_key, m.card_order, s.name"
    ))
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut settings = serde_json::Map::new();
    let mut masked: Vec<String> = Vec::new();
    for m in &rows {
        if m.secret && !plaintext {
            settings.insert(
                m.name.clone(),
                serde_json::Value::String(String::new()),
            );
            masked.push(m.name.clone());
        } else {
            settings.insert(
                m.name.clone(),
                serde_json::Value::String(m.value.clone()),
            );
        }
    }
    let count = settings.len();
    Ok(ok(serde_json::json!({
        "format": "fluxtorrent.settings",
        "version": 1,
        "exported_at": chrono::Utc::now(),
        "plaintext": plaintext,
        "count": count,
        "masked_secrets": masked,
        "settings": settings,
    })))
}

#[derive(Deserialize)]
struct ImportReq {
    settings: HashMap<String, String>,
    #[serde(default)]
    confirm: bool,
    /// 快照是否来自明文导出（`plaintext: true`）：决定密文空值的语义，
    /// 见模块顶部注释。缺省 false = 空值保持不变（安全默认）。
    #[serde(default)]
    plaintext: bool,
}

#[post("/admin/settings/import")]
async fn settings_import(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ImportReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // 导入仅 sysop（§4.3）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if body.settings.is_empty() {
        return Err(DomainError::Validation("导入内容为空".into()));
    }
    if body.settings.len() > MAX_BATCH * 4 {
        return Err(DomainError::Validation("导入条目过多".into()));
    }

    let names: Vec<String> = body.settings.keys().cloned().collect();
    let metas: Vec<MetaRow> =
        sqlx::query_as(&format!("{META_SELECT} WHERE s.name = ANY($1)"))
            .bind(&names)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let by_name: HashMap<String, MetaRow> =
        metas.into_iter().map(|m| (m.name.clone(), m)).collect();

    let mut unknown: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();
    let mut pending: Vec<(String, String, String)> = Vec::new(); // (name, old, new)

    for name in &names {
        let Some(meta) = by_name.get(name) else {
            unknown.push(name.clone());
            continue;
        };
        let raw = &body.settings[name];
        if meta.readonly {
            // 只读项（版本信息等）会随导出快照一并带回：导入时静默跳过而非报错
            skipped.push(name.clone());
            continue;
        }
        if meta.secret && raw.trim().is_empty() && !body.plaintext {
            continue; // 掩码快照中的密文空值 = 保持不变（明文快照则按字面还原）
        }
        match validate_field(meta, raw) {
            Ok(normalized) => {
                if normalized != meta.value {
                    pending.push((
                        name.clone(),
                        meta.value.clone(),
                        normalized,
                    ));
                }
            }
            Err(e) => errors.push((name.clone(), e)),
        }
    }
    if !errors.is_empty() {
        return Err(DomainError::FieldErrors(errors));
    }

    // 空跑：只回 diff，不落库（「导入后强制 diff 确认」）
    if !body.confirm {
        let diff: Vec<serde_json::Value> = pending
            .iter()
            .map(|(n, o, v)| {
                let secret = by_name[n].secret;
                serde_json::json!({
                    "name": n,
                    "group": by_name[n].grp,
                    "old": if secret { "••••".to_string() } else { truncate(o) },
                    "new": if secret { "••••".to_string() } else { truncate(v) },
                })
            })
            .collect();
        let changed_names: Vec<String> =
            pending.iter().map(|(n, _, _)| n.clone()).collect();
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "unknown": unknown,
            "skipped_readonly": skipped,
            "will_change": pending.len(),
            "diff": diff,
            "effects": effects_for(&changed_names),
        })));
    }

    // ---- 确认落库：事务 + 逐项审计 ----
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let mut changed: Vec<String> = Vec::new();
    let mut groups: Vec<String> = Vec::new();
    for (name, _old, new) in &pending {
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
        sqlx::query("UPDATE site_settings SET value = $2, updated_at = now() WHERE name = $1")
            .bind(name)
            .bind(new)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let meta = &by_name[name];
        let (audit_old, audit_new) = if meta.secret {
            ("已设置".to_string(), "已更新".to_string())
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
                "group": meta.grp,
                "old": audit_old,
                "new": audit_new,
                "via": "import",
            })
            .to_string(),
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        push_unique(&mut groups, &meta.grp);
        changed.push(name.clone());
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    for g in &groups {
        invalidate_cache(&state, g).await;
    }
    Ok(ok(serde_json::json!({
        "dry_run": false,
        "unknown": unknown,
        "skipped_readonly": skipped,
        "applied": changed.len(),
        "changed": changed,
        "groups": groups,
        "effects": effects_for(&changed),
    })))
}
