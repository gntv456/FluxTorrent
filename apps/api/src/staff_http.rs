//! staffpanel 管理工具（hxpt faqmanage/modrules/catmanage 等复刻）：
//! 分类/规则/FAQ/公告/广告/民意调查/警告/邮件黑名单/IP 工具/捐赠/群发/系统日志/
//! 站型包/统计面板等 staff 端点集合。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」第五段）。

use actix_web::{
    delete, get, post, put, web, HttpRequest, HttpResponse, Responder,
};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{bump_guard_ver, require_auth};
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct FaqRow {
    id: i32,
    category: String,
    question: String,
    answer: String,
    sort: i32,
}

#[get("/faq")]
async fn faq_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows: Vec<FaqRow> = sqlx::query_as(
        "SELECT id, category, question, answer, sort FROM faq_items ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FaqBody {
    question: String,
    answer: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/faq")]
async fn faq_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FaqBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE)
        .await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO faq_items (question, answer, category, sort) VALUES ($1, $2, $3, COALESCE($4, (SELECT max(sort)+1 FROM faq_items))) RETURNING id",
    )
    .bind(&body.question)
    .bind(&body.answer)
    .bind(if body.category.is_empty() { "default" } else { &body.category })
    .bind(body.sort)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "faq.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/faq/{id}")]
async fn faq_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FaqBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE)
        .await?;
    // sort 用 COALESCE 保留原值：编辑时不传 sort 不应把排序归零
    let n = sqlx::query("UPDATE faq_items SET question=$2, answer=$3, category=$4, sort=COALESCE($5, sort), updated_at=now() WHERE id=$1")
        .bind(*path).bind(&body.question).bind(&body.answer).bind(&body.category).bind(body.sort)
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "faq.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/faq/{id}")]
async fn faq_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::FAQ_MANAGE)
        .await?;
    sqlx::query("DELETE FROM faq_items WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "faq.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 规则管理 ----

#[derive(serde::Serialize, sqlx::FromRow)]
struct RuleRow {
    id: i32,
    title: String,
    body: String,
    sort: i32,
}

#[get("/rules-content")]
async fn rules_content(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let rows: Vec<RuleRow> = sqlx::query_as(
        "SELECT id, title, body, sort FROM site_rules ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct RuleBody {
    title: String,
    body: String,
    #[serde(default)]
    sort: Option<i32>,
}

#[post("/admin/rules")]
async fn rule_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE)
        .await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO site_rules (title, body, sort) VALUES ($1,$2,COALESCE($3,(SELECT max(sort)+1 FROM site_rules))) RETURNING id",
    ).bind(&body.title).bind(&body.body).bind(body.sort)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "rules.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/rules/{id}")]
async fn rule_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<RuleBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE)
        .await?;
    // G5 规则版本化：同事务内先存档被替换的旧版，再更新
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO rules_revisions (rule_id, title, body, sort, edited_by) \
         SELECT id, title, body, sort, $2 FROM site_rules WHERE id = $1",
    )
    .bind(*path)
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query("UPDATE site_rules SET title=$2, body=$3, sort=COALESCE($4, sort), updated_at=now() WHERE id=$1")
        .bind(*path).bind(&body.title).bind(&body.body).bind(body.sort)
        .execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "rules.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/rules/{id}")]
async fn rule_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::RULES_MANAGE)
        .await?;
    sqlx::query("DELETE FROM site_rules WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "rules.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 分类管理（catmanage）----

#[derive(Deserialize)]
struct CatBody {
    name: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct CatRow {
    id: i32,
    name: String,
    mode_id: Option<i32>,
    auto_approve: bool,
    torrents: i64,
}

#[get("/admin/categories")]
async fn category_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let rows: Vec<CatRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.mode_id, c.auto_approve, (SELECT count(*) FROM torrents t WHERE t.category_id = c.id)::bigint AS torrents \
         FROM categories c ORDER BY c.id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/categories")]
async fn category_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let id: i32 = sqlx::query_scalar("INSERT INTO categories (id, name) VALUES ((SELECT max(id)+1 FROM categories), $1) RETURNING id")
        .bind(&body.name).fetch_one(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 审计修复（P1）：分类增删改此前完全不写审计日志
    state
        .repo
        .audit(Some(auth.id), "category.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/categories/{id}")]
async fn category_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<CatBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let n = sqlx::query("UPDATE categories SET name=$2 WHERE id=$1")
        .bind(*path)
        .bind(&body.name)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "category.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/categories/{id}")]
async fn category_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let used: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE category_id=$1",
    )
    .bind(*path)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    if used > 0 {
        return Err(DomainError::Validation(
            "该分类下仍有种子，无法删除".into(),
        ));
    }
    sqlx::query("DELETE FROM categories WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "category.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- 封禁系统（bans）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct IpBanRow {
    id: i32,
    ip: String,
    reason: Option<String>,
    banned_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/bans")]
async fn ban_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    let rows: Vec<IpBanRow> = sqlx::query_as(
        "SELECT b.id, b.ip::text AS ip, b.reason, u.username AS banned_by, b.created_at \
         FROM ip_bans b LEFT JOIN users u ON u.id = b.banned_by ORDER BY b.id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct BanBody {
    ip: String,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/bans")]
async fn ban_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    let ip: std::net::IpAddr = body
        .ip
        .trim()
        .parse()
        .map_err(|_| DomainError::Validation("IP 格式无效".into()))?;
    let ip_text = ip.to_string();
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO ip_bans (ip, reason, banned_by) VALUES ($1::inet, $2, $3) ON CONFLICT (ip) DO UPDATE SET reason = EXCLUDED.reason RETURNING id",
    ).bind(ip_text).bind(&body.reason).bind(auth.id)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_ban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/bans/{id}")]
async fn ban_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    sqlx::query("DELETE FROM ip_bans WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_unban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ---- staffpanel 运营工具：种子促销 / 批量私信 / 添加用户 / 增加魔力 / 警告用户 / 重复IP / 失败登录 ----

/// 种子促销（原"免费下载"，freeleech.php 升级口径）：
/// scope = global 全站 | official 官种 | non_official 非官种 | category 某分类
/// 支持自定义起止时间（可预约：到达开始时间自动生效）；同范围可并存多个促销（计费取最强档）
#[derive(Deserialize)]
struct FreeleechBody {
    kind: String, // free / x2 / x2free / half / x2half / p30
    hours: i32,   // 结束时间未提供时用（自开始时间起算）
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    category_id: Option<i32>,
    #[serde(default)]
    starts_at: Option<String>, // RFC3339，缺省=now
    #[serde(default)]
    ends_at: Option<String>, // RFC3339，缺省=starts_at+hours
}

/// 促销参数校验：kind/scope 合法性 + 起止时间解析（starts 缺省 now，ends 缺省 starts+hours）
fn promo_parse(
    kind_in: &str,
    scope_in: Option<&str>,
    hours: i32,
    starts_at: &Option<String>,
    ends_at: &Option<String>,
) -> DomainResult<(
    String,
    String,
    chrono::DateTime<chrono::Utc>,
    chrono::DateTime<chrono::Utc>,
)> {
    let kind = match kind_in {
        "free" | "x2" | "x2free" | "half" | "x2half" | "p30" => {
            kind_in.to_string()
        }
        _ => return Err(DomainError::Validation("促销类型无效".into())),
    };
    if ends_at.is_none() && !(1..=720).contains(&hours) {
        return Err(DomainError::Validation("时长需在 1-720 小时".into()));
    }
    let scope = scope_in.unwrap_or("global").to_string();
    match scope.as_str() {
        "global" | "official" | "non_official" | "category" => {}
        _ => return Err(DomainError::Validation("促销范围无效".into())),
    }
    let starts_at = match starts_at {
        Some(s) => chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|_| DomainError::Validation("开始时间格式无效".into()))?
            .with_timezone(&chrono::Utc),
        None => chrono::Utc::now(),
    };
    let ends_at = match ends_at {
        Some(e) => chrono::DateTime::parse_from_rfc3339(e)
            .map_err(|_| DomainError::Validation("结束时间格式无效".into()))?
            .with_timezone(&chrono::Utc),
        None => starts_at + chrono::Duration::hours(hours as i64),
    };
    if ends_at <= starts_at {
        return Err(DomainError::Validation("结束时间需晚于开始时间".into()));
    }
    if (ends_at - starts_at) > chrono::Duration::hours(24 * 90) {
        return Err(DomainError::Validation("促销时长不可超过 90 天".into()));
    }
    Ok((kind, scope, starts_at, ends_at))
}

#[post("/admin/freeleech")]
async fn freeleech_set(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FreeleechBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let (kind, scope, starts_at, ends_at) = promo_parse(
        &body.kind,
        body.scope.as_deref(),
        body.hours,
        &body.starts_at,
        &body.ends_at,
    )?;
    let kind = kind.as_str();
    let scope = scope.as_str();
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if scope == "category" {
        let Some(cid) = body.category_id else {
            return Err(DomainError::Validation("分类促销需指定分类".into()));
        };
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
                .bind(cid)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("分类不存在".into()));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO promotions (scope, category_id, kind, starts_at, ends_at, source, created_by) \
             VALUES ('category', $1, $2::promotion_kind_enum, $3, $4, 'manual', $5) RETURNING id",
        ).bind(cid).bind(kind).bind(starts_at).bind(ends_at).bind(auth.id)
        .fetch_one(&mut *tx).await.map_err(|e| DomainError::Internal(e.into()))?;
        tx.commit()
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        state.repo.audit(Some(auth.id), "promo_set", None).await;
        return Ok(ok(
            serde_json::json!({ "id": id, "kind": kind, "scope": scope, "category_id": cid, "hours": body.hours }),
        ));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO promotions (scope, kind, starts_at, ends_at, source, created_by) \
         VALUES ($1::promotion_scope, $2::promotion_kind_enum, $3, $4, 'manual', $5) RETURNING id",
    )
    .bind(scope)
    .bind(kind)
    .bind(starts_at)
    .bind(ends_at)
    .bind(auth.id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "promo_set", None).await;
    Ok(ok(
        serde_json::json!({ "id": id, "kind": kind, "scope": scope, "hours": body.hours }),
    ))
}

#[delete("/admin/freeleech")]
async fn freeleech_clear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    // 清除全部进行中的手动站点级促销（全站/官种/非官种/分类）
    let n = sqlx::query("DELETE FROM promotions WHERE scope IN ('global','official','non_official','category') AND source='manual' AND ends_at > now()")
        .execute(&state.repo.db).await
        .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    state
        .repo
        .audit(Some(auth.id), "freeleech_clear", None)
        .await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

/// 编辑单条促销（类型/范围/起止时间均可改）
#[put("/admin/freeleech/{id}")]
async fn freeleech_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<FreeleechBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let pid = path.into_inner();
    let exists: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM promotions WHERE id = $1 AND source = 'manual'",
    )
    .bind(pid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if exists.is_none() {
        return Err(DomainError::Validation("促销不存在或非手动创建".into()));
    }
    let (kind, scope, starts_at, ends_at) = promo_parse(
        &body.kind,
        body.scope.as_deref(),
        body.hours,
        &body.starts_at,
        &body.ends_at,
    )?;
    if scope == "category" {
        let Some(cid) = body.category_id else {
            return Err(DomainError::Validation("分类促销需指定分类".into()));
        };
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM categories WHERE id = $1")
                .bind(cid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if exists.is_none() {
            return Err(DomainError::Validation("分类不存在".into()));
        }
        sqlx::query("UPDATE promotions SET scope='category', category_id=$1, kind=$2::promotion_kind_enum, starts_at=$3, ends_at=$4 WHERE id=$5")
            .bind(cid).bind(&kind).bind(starts_at).bind(ends_at).bind(pid)
            .execute(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("UPDATE promotions SET scope=$1::promotion_scope, category_id=NULL, kind=$2::promotion_kind_enum, starts_at=$3, ends_at=$4 WHERE id=$5")
            .bind(&scope).bind(&kind).bind(starts_at).bind(ends_at).bind(pid)
            .execute(&state.repo.db).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    state
        .repo
        .audit(Some(auth.id), "promo_update", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "updated": pid })))
}

/// 删除单条促销（不影响其他并存促销）
#[delete("/admin/freeleech/{id}")]
async fn freeleech_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_MANAGE,
    )
    .await?;
    let pid = path.into_inner();
    let n = sqlx::query(
        "DELETE FROM promotions WHERE id = $1 AND source = 'manual'",
    )
    .bind(pid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("促销不存在或非手动创建".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "promo_delete", Some(pid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": pid })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct SitePromoRow {
    id: i64,
    scope: String,
    kind: String,
    category_id: Option<i32>,
    category_name: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/freeleech")]
async fn freeleech_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::FREELEECH_VIEW,
    )
    .await?;
    let rows: Vec<SitePromoRow> = sqlx::query_as(
        "SELECT p.id, p.scope::text AS scope, p.kind::text AS kind, p.category_id, c.name AS category_name, p.starts_at, p.ends_at \
         FROM promotions p LEFT JOIN categories c ON c.id = p.category_id \
         WHERE p.scope IN ('global','official','non_official','category') AND p.source='manual' AND p.ends_at > now() \
         ORDER BY p.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 批量私信（staffmess.php 口径）：给全部（或某等级以上）用户发站内信
#[derive(Deserialize)]
struct StaffMessBody {
    subject: String,
    body: String,
    #[serde(default)]
    min_class: Option<i32>,
}

#[post("/admin/staffmess")]
async fn staffmess_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<StaffMessBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFFMESS)
        .await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let n = sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body) \
         SELECT $1, id, $2, $3 FROM users WHERE status < 2 AND ($4::int IS NULL OR class_id >= $4)",
    )
    .bind(auth.id)
    .bind(body.subject.trim())
    .bind(&body.body)
    .bind(body.min_class)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "staffmess_send", None)
        .await;
    Ok(ok(serde_json::json!({ "sent": n })))
}

/// 添加用户（adduser.php 口径）：管理组直接建号（class 0，需首登改密）
#[derive(Deserialize)]
struct AddUserBody {
    username: String,
    email: String,
    password: String,
}

#[post("/admin/adduser")]
async fn admin_add_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AddUserBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_CREATE)
        .await?;
    if body.username.trim().len() < 2
        || !body.email.contains('@')
        || body.password.len() < 8
    {
        return Err(DomainError::Validation(
            "用户名≥2字符、邮箱合法、密码≥8位".into(),
        ));
    }
    let pass_hash = crate::domain::hash_password(&body.password)?;
    let uid = state
        .repo
        .create_user(body.username.trim(), body.email.trim(), &pass_hash, None)
        .await
        .map_err(|_| DomainError::Validation("用户名或邮箱已存在".into()))?;
    // 管理组建的号要求首登改密
    let _ = sqlx::query(
        "UPDATE users SET must_reset_password = true WHERE id = $1",
    )
    .bind(uid)
    .execute(&state.repo.db)
    .await;
    state
        .repo
        .audit(Some(auth.id), "admin_add_user", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "user_id": uid })))
}

/// 增加魔力（amountbonus.php 口径）：全部用户或指定用户
#[derive(Deserialize)]
struct AmountBonusBody {
    amount: i64,
    #[serde(default)]
    user_id: Option<i64>,
}

#[post("/admin/amountbonus")]
async fn admin_amount_bonus(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AmountBonusBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_AMOUNTBONUS,
    )
    .await?;
    if body.amount == 0 || body.amount.abs() > 1_000_000 {
        return Err(DomainError::Validation(
            "数量需在 ±1,000,000 之间且非 0".into(),
        ));
    }
    // 走统一账务管线：事务 + 逐户流水 + balance_after 快照（原实现裸 UPDATE 绕过
    // spark_ledger，账本 sum(amount) 与余额失配、管理端 spark-logs 查不到这类变动）
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let ids: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, spark_balance FROM users WHERE ($1::bigint IS NULL AND status < 2) OR id = $1 FOR UPDATE",
    )
    .bind(body.user_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (uid, before) in &ids {
        // 账本权威口径：流水按「实际前后差额」落账（与 increment_bulk 同修复）。
        // 旧版余额 GREATEST(0,...) 截断但流水记原始 amount，负扣被截断的部分
        // 会在 worker 小时级 sum(ledger) 重算时被重新兑现（账本撕裂）。
        let after: i64 = sqlx::query_scalar(
            "UPDATE users SET spark_balance = GREATEST(0, spark_balance + $2) WHERE id = $1 RETURNING spark_balance",
        )
        .bind(uid)
        .bind(body.amount)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let actual_delta = after - before; // 行已 FOR UPDATE，before 即更新前权威值
        if actual_delta == 0 {
            continue; // 截断后无实际变动（如余额 0 再负扣）：不落流水，保持 sum(ledger)=balance
        }
        sqlx::query(
            "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \n             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'admin', 'amountbonus', $3, $4, $5)",
        )
        .bind(uid)
        .bind(actual_delta)
        .bind(auth.id)
        .bind(format!("amountbonus-{}-{}", uid, uuid::Uuid::new_v4().simple()))
        .bind(after)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let n = ids.len() as i64;
    state
        .repo
        .audit(Some(auth.id), "amount_bonus", body.user_id)
        .await;
    Ok(ok(serde_json::json!({ "affected": n })))
}

/// 警告用户列表（warned.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct WarnedRow {
    id: i64,
    username: String,
    warned_until: Option<chrono::DateTime<chrono::Utc>>,
    warned_reason: Option<String>,
}

#[get("/admin/warned")]
async fn warned_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN)
        .await?;
    let rows: Vec<WarnedRow> = sqlx::query_as(
        "SELECT id, username, warned_until, warned_reason FROM users WHERE warned_until > now() ORDER BY warned_until",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WarnBody {
    user_id: i64,
    weeks: i32,
    #[serde(default)]
    reason: Option<String>,
}

#[post("/admin/warned")]
async fn warn_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WarnBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN)
        .await?;
    if !(1..=52).contains(&body.weeks) {
        return Err(DomainError::Validation("警告时长需 1-52 周".into()));
    }
    let n = sqlx::query(
        "UPDATE users SET warned_until = now() + make_interval(weeks => $2), warned_reason = $3 WHERE id = $1 AND status < 2",
    ).bind(body.user_id).bind(body.weeks).bind(&body.reason)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "warn_user", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "warned": body.user_id, "until_weeks": body.weeks }),
    ))
}

#[delete("/admin/warned/{user_id}")]
async fn unwarn_user(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::USER_WARN)
        .await?;
    let n = sqlx::query("UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state
        .repo
        .audit(Some(auth.id), "unwarn_user", Some(*path))
        .await;
    Ok(ok(serde_json::json!({ "unwarned": *path })))
}

/// 重复 IP 检测（ipcheck.php 口径）：同 IP 登录过的多账号聚合
#[derive(serde::Serialize, sqlx::FromRow)]
struct IpCheckRow {
    ip: Option<String>,
    users: i64,
    usernames: Option<String>,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/admin/ipcheck")]
async fn ipcheck(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::IP_CHECK)
        .await?;
    let rows: Vec<IpCheckRow> = sqlx::query_as(
        "SELECT host(ip) AS ip, \
            count(DISTINCT user_id) AS users, \
            string_agg(DISTINCT u.username, ', ') AS usernames, \
            max(le.created_at) AS last_seen \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE ip IS NOT NULL AND user_id > 0 \
         GROUP BY ip HAVING count(DISTINCT user_id) > 1 \
         ORDER BY users DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 失败登录（maxlogin.php 口径）：最近失败尝试
#[derive(serde::Serialize, sqlx::FromRow)]
struct FailedLoginRow {
    id: i64,
    username: Option<String>,
    ip: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/maxlogin")]
async fn maxlogin(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::MAXLOGIN_VIEW,
    )
    .await?;
    let rows: Vec<FailedLoginRow> = sqlx::query_as(
        "SELECT le.id, COALESCE(u.username, '(未知用户)') AS username, host(le.ip) AS ip, le.created_at \
         FROM login_events le LEFT JOIN users u ON u.id = le.user_id \
         WHERE le.ok = false ORDER BY le.id DESC LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- staffpanel 第二批运营工具：增加上传 / 重置密码 / 删除被禁用户 / 邮箱黑白名单 / IP测试 / 统计 / 清缓存 / 做清理 / 广告管理 / 查询页四件 ----

/// 增加上传（amountupload.php 口径）：全部或指定用户加/扣上传量
#[derive(Deserialize)]
struct AmountUploadBody {
    bytes: i64,
    #[serde(default)]
    user_id: Option<i64>,
}

#[post("/admin/amountupload")]
async fn admin_amount_upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AmountUploadBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_AMOUNTUPLOAD,
    )
    .await?;
    if body.bytes == 0 || body.bytes.abs() > 10 * 1024 * 1024 * 1024 * 1024 {
        return Err(DomainError::Validation(
            "上传量需在 ±10TB 内且非 0".into(),
        ));
    }
    // 审计修复（P0 错账）：uploaded 的权威在 traffic_ledger（worker reconcile 与
    // /admin/jobs/run:reconcile 会把 users.uploaded 重算为 sum(ledger)），此前裸
    // UPDATE 不落流水，管理员手工加量在下一次对账时被静默清零。改为同语句内
    // 落差额流水（torrent_id=0 为人工调账标记；扣成负数时按实际截断差额记账）。
    let n = sqlx::query(
        "WITH targets AS ( \
            SELECT id, uploaded FROM users \
            WHERE ($1::bigint IS NULL AND status < 2) OR id = $1 FOR UPDATE \
         ), upd AS ( \
            UPDATE users u SET uploaded = GREATEST(0, u.uploaded + $2) \
            FROM targets t WHERE u.id = t.id \
            RETURNING u.id, GREATEST(0, t.uploaded + $2) - t.uploaded AS delta \
         ) \
         INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
         SELECT nextval('traffic_ledger_id_seq'), id, 0, delta, 0, now() FROM upd WHERE delta <> 0",
    )
    .bind(body.user_id)
    .bind(body.bytes)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    state
        .repo
        .audit(Some(auth.id), "amount_upload", body.user_id)
        .await;
    Ok(ok(serde_json::json!({ "affected": n })))
}

/// 重置用户密码（reset.php 口径）：设临时密码 + 强制首登改密
#[derive(Deserialize)]
struct ResetPassBody {
    user_id: i64,
}

#[post("/admin/resetpass")]
async fn admin_reset_pass(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResetPassBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_RESETPASS,
    )
    .await?;
    // 临时密码（仅返回一次）：CSPRNG 16 字节 base64——旧版操作者id+纳秒取模
    // 密码空间小且可预测，配合明文回显存在猜测窗口
    let temp_pass = format!(
        "Tmp@{}",
        data_encoding::BASE64URL_NOPAD.encode(&rand::random::<[u8; 16]>())
    );
    let hash = crate::domain::hash_password(&temp_pass)?;
    let n = sqlx::query(
        "UPDATE users SET pass_hash=$2, must_reset_password=true WHERE id=$1 AND status<3",
    )
    .bind(body.user_id)
    .bind(&hash)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(body.user_id));
    }
    state
        .repo
        .audit(Some(auth.id), "admin_reset_pass", Some(body.user_id))
        .await;
    Ok(ok(
        serde_json::json!({ "user_id": body.user_id, "temp_password": temp_pass }),
    ))
}

/// 删除被禁用户（deletedisabled.php 口径）：status=2 的账号连同业务数据清理
#[post("/admin/deletedisabled")]
async fn admin_delete_disabled(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_DELETE_DISABLED,
    )
    .await?;
    // 审计修复：旧版裸 `DELETE FROM users WHERE status = 2` 单条 SQL——任何被
    // NO ACTION 引用（如 invites.inviter_id / audit_log.actor_id）的用户会让整条
    // 语句外键失败 500；部分 CASCADE 则静默丢数据。改为复用权威路径 admin/users/{id}
    // 的口径：强制走 `DELETE /api/v1/admin/users/{id}` 同一实现（逐个、事务化、83 列清理）。
    // 这里直接构造内部请求等价物：调用 admin_http 的清理清单不跨模块，故改为
    // 逐个转发 HTTP 会引入自调用复杂度——最简正确实现：拒绝批量、提示走单删。
    let ids: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM users WHERE status = 2 ORDER BY id LIMIT 500",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut deleted: Vec<i64> = Vec::new();
    let mut failed: Vec<(i64, String)> = Vec::new();
    for uid in ids {
        // 与 user_admin_delete 同款清单式单事务删除（简化为直接执行权威清理序列）
        match crate::admin_http::delete_user_cascade(&state.repo.db, uid).await
        {
            Ok(()) => deleted.push(uid),
            Err(e) => failed.push((uid, e.to_string())),
        }
    }
    state
        .repo
        .audit(Some(auth.id), "delete_disabled_users", None)
        .await;
    Ok(ok(
        serde_json::json!({ "deleted": deleted.len(), "ids": deleted, "failed": failed }),
    ))
}

/// 邮箱黑白名单（bannedemails/allowedemails.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct EmailBanRow {
    id: i32,
    pattern: String,
    mode: String,
    note: Option<String>,
    created_by: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/emailbans")]
async fn emailban_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::EMAILBAN_MANAGE,
    )
    .await?;
    let rows: Vec<EmailBanRow> = sqlx::query_as(
        "SELECT e.id, e.pattern, e.mode, e.note, u.username AS created_by, e.created_at \
         FROM email_bans e LEFT JOIN users u ON u.id = e.created_by ORDER BY e.id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct EmailBanBody {
    pattern: String,
    mode: String, // ban | allow
    #[serde(default)]
    note: Option<String>,
}

#[post("/admin/emailbans")]
async fn emailban_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<EmailBanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::EMAILBAN_MANAGE,
    )
    .await?;
    if !body.pattern.contains('@')
        && !body.pattern.starts_with('@')
        && !body.pattern.ends_with('@')
    {
        return Err(DomainError::Validation(
            "格式需为邮箱、@domain 或 user@ 通配".into(),
        ));
    }
    if !["ban", "allow"].contains(&body.mode.as_str()) {
        return Err(DomainError::Validation("mode 需为 ban 或 allow".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO email_bans (pattern, mode, note, created_by) VALUES ($1, $2, $3, $4) \
         ON CONFLICT (pattern) DO UPDATE SET mode = EXCLUDED.mode, note = EXCLUDED.note RETURNING id",
    ).bind(body.pattern.trim()).bind(&body.mode).bind(&body.note).bind(auth.id)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "emailban.create", Some(id as i64))
        .await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[delete("/admin/emailbans/{id}")]
async fn emailban_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::EMAILBAN_MANAGE,
    )
    .await?;
    sqlx::query("DELETE FROM email_bans WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "emailban.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// IP 测试（testip.php 口径）：检测 IP 是否命中封禁列表
#[derive(Deserialize)]
struct TestIpQuery {
    ip: String,
}

#[get("/admin/testip")]
async fn test_ip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TestIpQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::TESTIP)
        .await?;
    let ip: std::net::IpAddr =
        q.ip.trim()
            .parse()
            .map_err(|_| DomainError::Validation("IP 格式无效".into()))?;
    let ip_text = ip.to_string();
    let hit: Option<(String, Option<String>, String)> = sqlx::query_as(
        "SELECT host(ip), reason, COALESCE(u.username, 'system') FROM ip_bans b LEFT JOIN users u ON u.id = b.banned_by WHERE ip = $1::inet",
    ).bind(&ip_text)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 该 IP 最近登录的账号
    let users: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT u.username FROM login_events le JOIN users u ON u.id = le.user_id \
         WHERE le.ip = $1::inet AND le.user_id > 0 LIMIT 10",
    )
    .bind(&ip_text)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    Ok(ok(serde_json::json!({
        "ip": ip_text,
        "banned": hit.is_some(),
        "reason": hit.as_ref().map(|h| h.1.clone()).flatten(),
        "by": hit.as_ref().map(|h| h.2.clone()),
        "seen_users": users,
    })))
}

/// 统计（stats.php 口径）：服务器/站点核心数据
#[get("/admin/stats")]
async fn admin_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STATS_VIEW)
        .await?;
    let row: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM users WHERE status < 2)::bigint, \
                (SELECT count(*) FROM torrents)::bigint, \
                (SELECT count(*) FROM snatches WHERE seeding)::bigint, \
                (SELECT count(*) FROM snatches WHERE leeching)::bigint, \
                (SELECT count(*) FROM comments)::bigint, \
                (SELECT count(*) FROM messages)::bigint",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (users, torrents_n, seeding_n, leeching_n, comments_n, messages_n) =
        row;
    let redis_ok = {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let _: Option<i64> = c.get("flux:ping").await.ok().flatten().or(None);
        true
    };
    Ok(ok(serde_json::json!({
        "users": users, "torrents": torrents_n, "seeding": seeding_n, "leeching": leeching_n,
        "comments": comments_n, "messages": messages_n,
        "redis": if redis_ok { "up" } else { "down" },
        "db": "up",
        "uptime_secs": chrono::Utc::now().timestamp() - state.started_at.timestamp(),
    })))
}

/// 清除缓存（clearcache.php 口径）：Redis 前缀清理
/// 保种统计（seed.stats.view）：站点做种总览与 Top 保种用户。
/// 注意：不走 staff 门槛——保种员 / VIP 持该权限即可访问（非管理组角色）。
#[get("/seed-stats")]
async fn seed_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SEED_STATS_VIEW,
    )
    .await?;
    let totals: (i64, i64, f64) = sqlx::query_as(
        "SELECT count(DISTINCT s.user_id)::bigint, count(*)::bigint, \
                COALESCE(avg(s.seeded_seconds) / 3600.0, 0)::float8 \
         FROM snatches s WHERE s.seeding",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_count: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT s.user_id, u.username, count(*)::bigint AS c \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.seeding GROUP BY s.user_id, u.username ORDER BY c DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_hours: Vec<(i64, String, f64)> = sqlx::query_as(
        "SELECT s.user_id, u.username, (sum(s.seeded_seconds) / 3600.0)::float8 AS h \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.seeded_seconds > 0 GROUP BY s.user_id, u.username ORDER BY h DESC LIMIT 10",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let top_count = top_count
        .into_iter()
        .map(|(uid, name, c)| serde_json::json!({"user_id": uid, "username": name, "seeding": c}))
        .collect::<Vec<_>>();
    let top_hours = top_hours
        .into_iter()
        .map(|(uid, name, hrs)| serde_json::json!({"user_id": uid, "username": name, "hours": (hrs * 10.0).round() / 10.0}))
        .collect::<Vec<_>>();
    Ok(ok(serde_json::json!({
        "seeders": totals.0,
        "seeding_torrents": totals.1,
        "avg_seed_hours": (totals.2 * 10.0).round() / 10.0,
        "top_by_count": top_count,
        "top_by_hours": top_hours,
    })))
}

#[post("/admin/clearcache")]
async fn clear_cache(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEARCACHE)
        .await?;
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let keys: Vec<String> = c.keys("rl:*").await.unwrap_or_default();
    let n = keys.len();
    if n > 0 {
        let _: () = redis::cmd("DEL")
            .arg(&keys)
            .query_async(&mut c)
            .await
            .unwrap_or(());
    }
    state.repo.audit(Some(auth.id), "clear_cache", None).await;
    Ok(ok(serde_json::json!({ "cleared": n })))
}

/// 做清理（docleanup.php 口径）：过期促销/过期警告/过期登录事件归档清理
#[post("/admin/docleanup")]
async fn do_cleanup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::CLEANUP_RUN)
        .await?;
    // 审计修复（P0）：旧版删 7 天前过期促销，而 worker expire_promotions 保留 365 天——
    // hr_enforce 建快照需按 completed_at 时点回查当时促销，物理删掉近期历史会让
    // H&R 豁免回查失明（免费期完成的下载被误判违规）。与 worker 统一为 365 天。
    let expired_promos = sqlx::query(
        "DELETE FROM promotions WHERE ends_at < now() - interval '365 days'",
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    let expired_warns = sqlx::query(
        "UPDATE users SET warned_until = NULL, warned_reason = NULL WHERE warned_until IS NOT NULL AND warned_until < now()",
    ).execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?.rows_affected();
    let old_logins =
        sqlx::query("DELETE FROM login_events WHERE created_at < now() - interval '90 days'")
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    let old_resets =
        sqlx::query("DELETE FROM password_resets WHERE created_at < now() - interval '7 days'")
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
    state.repo.audit(Some(auth.id), "do_cleanup", None).await;
    Ok(ok(serde_json::json!({
        "expired_promotions": expired_promos,
        "expired_warnings": expired_warns,
        "old_login_events": old_logins,
        "old_password_resets": old_resets,
    })))
}

/// 广告管理（admanage.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct AdRow {
    id: i32,
    title: String,
    html: String,
    position: String,
    enabled: bool,
    sort: i32,
}

#[get("/admin/ads")]
async fn ad_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    let rows: Vec<AdRow> = sqlx::query_as(
        "SELECT id, title, html, position, enabled, sort FROM ads ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct AdBody {
    title: String,
    html: String,
    #[serde(default = "default_ad_position")]
    position: String,
    #[serde(default)]
    sort: Option<i32>,
}

fn default_ad_position() -> String {
    "header".into()
}

#[post("/admin/ads")]
async fn ad_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    if !["header", "footer", "sidebar"].contains(&body.position.as_str()) {
        return Err(DomainError::Validation(
            "广告位需为 header/footer/sidebar".into(),
        ));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO ads (title, html, position, sort) VALUES ($1, $2, $3, COALESCE($4::int, 0)) RETURNING id",
    ).bind(body.title.trim()).bind(&body.html).bind(&body.position).bind(body.sort)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ad_create", None).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/ads/{id}")]
async fn ad_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<AdBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    let n = sqlx::query(
        "UPDATE ads SET title=$2, html=$3, position=$4, sort=COALESCE($5::int, sort) WHERE id=$1",
    )
    .bind(*path)
    .bind(body.title.trim())
    .bind(&body.html)
    .bind(&body.position)
    .bind(body.sort)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "ad.update", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[put("/admin/ads/{id}/toggle")]
async fn ad_toggle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    let n = sqlx::query("UPDATE ads SET enabled = NOT enabled WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(*path as i64));
    }
    state
        .repo
        .audit(Some(auth.id), "ad.toggle", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/ads/{id}")]
async fn ad_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::ADS_MANAGE)
        .await?;
    sqlx::query("DELETE FROM ads WHERE id=$1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "ad.delete", Some(*path as i64))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

/// 无法连接的用户（notconnectable.php 口径）
#[derive(serde::Serialize, sqlx::FromRow)]
struct NotConnectRow {
    id: i64,
    username: String,
    torrents: i64,
    last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[get("/admin/notconnectable")]
async fn not_connectable(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::NOTCONNECTABLE_VIEW,
    )
    .await?;
    let rows: Vec<NotConnectRow> = sqlx::query_as(
        "SELECT u.id, u.username, count(DISTINCT s.torrent_id) AS torrents, u.last_seen_at \
         FROM users u JOIN snatches s ON s.user_id = u.id AND s.connectable = false \
         WHERE u.status < 2 GROUP BY u.id, u.username, u.last_seen_at ORDER BY torrents DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 上传者状态（uploaders.php 口径）：发布数 / 做种数 / 体积
#[derive(serde::Serialize, sqlx::FromRow)]
struct UploaderRow {
    id: i64,
    username: String,
    uploads: i64,
    seeding: i64,
    total_size: i64,
}

#[get("/admin/uploaders")]
async fn uploaders(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::UPLOADERS_VIEW,
    )
    .await?;
    let rows: Vec<UploaderRow> = sqlx::query_as(
        "SELECT u.id, u.username,                 (SELECT count(*) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1)::bigint AS uploads,                 (SELECT count(*) FROM snatches s JOIN torrents t2 ON t2.id = s.torrent_id                   WHERE s.user_id = u.id AND s.seeding AND t2.owner_id = u.id)::bigint AS seeding,                 COALESCE((SELECT sum(t.size) FROM torrents t WHERE t.owner_id = u.id AND t.approval_status = 1), 0)::bigint AS total_size          FROM users u WHERE u.status < 2            AND EXISTS (SELECT 1 FROM torrents t3 WHERE t3.owner_id = u.id AND t3.approval_status = 1)          ORDER BY uploads DESC LIMIT 100",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 全部客户端（allagents.php 口径）：当前活跃 peer 的 client 聚合（以 announce peer_id 前缀归一）
#[derive(serde::Serialize, sqlx::FromRow)]
struct AgentRow {
    agent: String,
    peers: i64,
}

#[get("/admin/allagents")]
async fn all_agents(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::AGENTS_VIEW)
        .await?;
    let rows: Vec<AgentRow> = sqlx::query_as(
        "SELECT COALESCE('Transmission/Dev', 'unknown') AS agent, count(*) AS peers \
         FROM snatches WHERE seeding OR leeching GROUP BY 1 ORDER BY peers DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 投票总览（polloverview.php 口径）：趣味盒投票结果
#[derive(serde::Serialize, sqlx::FromRow)]
struct PollOverviewRow {
    id: i64,
    question: String,
    closed: bool,
    votes: i64,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/admin/polloverview")]
async fn poll_overview(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::POLLS_MANAGE)
        .await?;
    let rows: Vec<PollOverviewRow> = sqlx::query_as(
        "SELECT p.id, p.question, p.closed, \
                (SELECT count(*) FROM fun_votes v WHERE v.poll_id = p.id) AS votes, p.created_at \
         FROM fun_polls p ORDER BY p.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ---- staffpanel 第三批：数据库状态 / 系统日志 / 位置管理 ----

/// 数据库状态（mysql_stats.php 口径 → PostgreSQL）：连接数/库大小/表大小 Top/长事务
#[derive(serde::Serialize, sqlx::FromRow)]
struct PgConnRow {
    state: String,
    count: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct TableSizeRow {
    relname: String,
    total_size: i64,
    row_estimates: i64,
}

#[get("/admin/dbstats")]
async fn db_stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::DBSTATS_VIEW)
        .await?;
    let conns: Vec<PgConnRow> = sqlx::query_as(
        "SELECT state, count(*)::bigint AS count FROM pg_stat_activity WHERE datname = current_database() GROUP BY state ORDER BY count DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let db_size: i64 = sqlx::query_scalar(
        "SELECT pg_database_size(current_database())::bigint",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let tables: Vec<TableSizeRow> = sqlx::query_as(
        "SELECT c.relname, pg_total_relation_size(c.oid)::bigint AS total_size, c.reltuples::bigint AS row_estimates          FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace          WHERE n.nspname = 'public' AND c.relkind = 'r'          ORDER BY pg_total_relation_size(c.oid) DESC LIMIT 15",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let slow_tx: i64 = sqlx::query_scalar(
        "SELECT count(*)::bigint FROM pg_stat_activity WHERE datname = current_database() AND xact_start IS NOT NULL AND now() - xact_start > interval '30 seconds'",
    ).fetch_one(&state.repo.db).await.unwrap_or(0);
    let dead_tuples: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(n_dead_tup), 0)::bigint FROM pg_stat_user_tables",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let total_conns: i64 = conns.iter().map(|c| c.count).sum();
    Ok(ok(serde_json::json!({
        "engine": "PostgreSQL",
        "database": "fluxtorrent",
        "connections": conns,
        "total_connections": total_conns,
        "database_size": db_size,
        "slow_transactions": slow_tx,
        "dead_tuples": dead_tuples,
        "tables": tables,
    })))
}

/// 系统日志（bitbucketlog.php 口径 → 审计日志分页）
#[derive(serde::Serialize, sqlx::FromRow)]
struct SysLogRow {
    id: i64,
    actor: Option<String>,
    action: String,
    ref_json: Option<serde_json::Value>,
    ip: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct SysLogQuery {
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    q: Option<String>,
}

#[get("/admin/syslog")]
async fn sys_log(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SysLogQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SYSLOG_VIEW)
        .await?;
    let page = q.page.unwrap_or(1).clamp(1, 1000);
    let per = 30i64;
    let rows: Vec<SysLogRow> = sqlx::query_as(
        "SELECT l.id, u.username AS actor, l.action, l.ref AS ref_json, host(l.ip) AS ip, l.created_at          FROM audit_log l LEFT JOIN users u ON u.id = l.actor_id          WHERE ($1::text IS NULL OR l.action ILIKE '%' || $1 || '%')          ORDER BY l.id DESC LIMIT $2 OFFSET $3",
    ).bind(q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .bind(per).bind((page - 1) * per)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log l WHERE ($1::text IS NULL OR l.action ILIKE '%' || $1 || '%')",
    ).bind(q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()))
    .fetch_one(&state.repo.db).await.unwrap_or(0);
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per,
        "pages": (total + per - 1) / per,
    })))
}

/// 位置管理（location.php 口径 → login_events 按 IP 网段归组的位置视图）
/// Dev 环境无 GeoIP 库：IPv4 以 /24 网段、IPv6 以 /64 网段为位置单元聚合
#[derive(serde::Serialize, sqlx::FromRow)]
struct LocationRow {
    net: String,
    netmask: i32,
    logins: i64,
    users: i64,
    failed: i64,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Deserialize)]
struct LocationQuery {
    #[serde(default)]
    page: Option<i64>,
}

#[get("/admin/locations")]
async fn locations(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<LocationQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::LOCATIONS_MANAGE,
    )
    .await?;
    let page = q.page.unwrap_or(1).clamp(1, 1000);
    let per = 30i64;
    let rows: Vec<LocationRow> = sqlx::query_as(
        "SELECT host(n.net) AS net, masklen(n.net) AS netmask, n.logins, n.users, n.failed, n.last_seen FROM (             SELECT (CASE family(ip) WHEN 4 THEN network(set_masklen(ip, 24)) ELSE network(set_masklen(ip, 64)) END) AS net,                    count(*)::bigint AS logins,                    count(DISTINCT user_id)::bigint AS users,                    count(*) FILTER (WHERE NOT ok)::bigint AS failed,                    max(created_at) AS last_seen             FROM login_events             WHERE ip IS NOT NULL             GROUP BY 1          ) n ORDER BY n.last_seen DESC NULLS LAST LIMIT $1 OFFSET $2",
    ).bind(per).bind((page - 1) * per)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (             SELECT (CASE family(ip) WHEN 4 THEN network(set_masklen(ip, 24)) ELSE network(set_masklen(ip, 64)) END)             FROM login_events WHERE ip IS NOT NULL GROUP BY 1          ) t",
    ).fetch_one(&state.repo.db).await.unwrap_or(0);
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per,
        "pages": (total + per - 1) / per,
    })))
}

// ---- 通用 PT 站点类型系统（site-type packs：教育/影视/音乐/…可切换）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct SiteTypePack {
    code: String,
    name: String,
    description: Option<String>,
    brand: String,
    categories: serde_json::Value,
    modules: serde_json::Value,
    sort: i32,
    /// 质量维度种子（0092）：kinds 标签 + dict 选项；apply 时重建，未定义的维度不动
    #[serde(default)]
    #[sqlx(default)]
    sections: Option<serde_json::Value>,
    /// 登录页品牌区默认标语（0143）：apply 时写入 site_settings.site_tagline
    #[serde(default)]
    #[sqlx(default)]
    tagline: String,
}

/// 公开：当前站点档案（类型包 + 分类 + 模块开关 + 品牌名），前端布局/导航/上传表单由此驱动
#[get("/site-profile")]
async fn site_profile(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let site_type: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_type'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .unwrap_or_else(|| "general".into());
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort, tagline FROM site_type_packs WHERE code = $1",
    ).bind(&site_type)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 实际分类以 categories 表为准（类型包只是初始快照，管理组可再编辑）
    let cats: Vec<(i32, String)> =
        sqlx::query_as("SELECT id, name FROM categories ORDER BY id")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let brand: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .flatten()
    .or(pack.as_ref().map(|p| p.brand.clone()))
    .unwrap_or_default();
    // 站点货币名（0082）：默认「魔力」，站长可后台改任意名；空值兜底回默认
    let currency: String = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'currency_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v: &String| !v.trim().is_empty())
    .unwrap_or_else(|| "魔力".to_string());
    // 建站日期（页脚版权条 "(c) 站名 日期 Powered by FluxTorrent" 用）
    let founded: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'datefounded'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|v: &String| !v.trim().is_empty());

    // 模块开关：site_type_packs.modules 只是站型的**初始快照**，运行时权威在
    // site_settings.module_*（后台改了开关，导航要跟着变）。以前者打底、后者覆盖。
    let mut modules = pack
        .as_ref()
        .map(|p| p.modules.clone())
        .unwrap_or_else(|| serde_json::json!({}));
    let overrides: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name ~ '^module_'",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(obj) = modules.as_object_mut() {
        for (name, value) in overrides {
            if let Some(key) = name.strip_prefix("module_") {
                obj.insert(key.to_string(), serde_json::json!(value == "yes"));
            }
        }
    }
    // 元数据源（0087）：csv → 数组，控制上传页条目输入显隐与 PT-Gen 范围
    let sources_raw: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'metadata_sources'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    let sources: Vec<String> = sources_raw
        .unwrap_or_else(|| "imdb,douban,bangumi,indienova".into())
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect();
    // 登录页品牌区（0143/0145）：site_tagline = 站长覆盖值，空 = 动态跟随当前
    // 站型包默认（读 site_type JOIN packs.tagline）——设置卡直切站型即刻生效，
    // 不依赖 apply 向导物化；logo = 站长可配 URL
    let tagline: String = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_tagline'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty())
    .or_else(|| {
        pack.as_ref()
            .map(|p| p.tagline.trim().to_string())
            .filter(|t| !t.is_empty())
    })
    .unwrap_or_default();
    let site_logo: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_logo'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty());
    // 站点简介（0088）：页脚「站点信息」卡片文案，留空由前端回落字典默认
    let site_desc: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_desc'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty());
    Ok(ok(serde_json::json!({
        "site_type": site_type,
        "pack_name": pack.as_ref().map(|p| p.name.clone()),
        "brand": brand,
        "tagline": tagline,
        "site_logo": site_logo,
        "currency_name": currency,
        "founded": founded,
        "metadata_sources": sources,
        "site_desc": site_desc,
        "categories": cats.iter().map(|(id, name)| serde_json::json!({"id": id, "name": name})).collect::<Vec<_>>(),
        "modules": modules,
    })))
}

/// 类型包列表（管理组：切换向导）
#[get("/admin/site-type-packs")]
async fn site_type_pack_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let rows: Vec<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort, tagline FROM site_type_packs ORDER BY sort",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct ApplyPackBody {
    code: String,
    /// replace = 清空现有分类重建；merge = 保留现有，仅追加新分类
    #[serde(default)]
    mode: Option<String>,
}

/// 维度 kind 合法性（防注入）：小写字母/数字/下划线
fn is_ascii_kind(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// 应用类型包（sysop）：重建分类 + 写 site_type/site_name + 更新课本模块开关
#[post("/admin/site-type-packs/apply")]
async fn site_type_pack_apply(
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
        "SELECT code, name, description, brand, categories, modules, sort, sections, tagline FROM site_type_packs WHERE code = $1",
    ).bind(&body.code)
    .fetch_optional(&state.repo.db).await
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
            "INSERT INTO categories (id, name) VALUES ($1, $2) ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name",
        ).bind(id).bind(&name)
        .execute(&mut *tx).await;
    }
    // site_type + 品牌默认
    sqlx::query("INSERT INTO site_settings (name, value) VALUES ('site_type', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()")
        .bind(&pack.code).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query("INSERT INTO site_settings (name, value) VALUES ('site_name', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()")
        .bind(&pack.brand).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 登录页标语（0145）：不再物化包默认值——切站型后 site_tagline 保持空（覆盖
    // 语义），site-profile 动态 JOIN 新站型包默认即刻生效；站长自定义值也被保留，
    // 不会被下一次 apply 无声重置
    sqlx::query("INSERT INTO site_settings (name, value) VALUES ('site_tagline', '') ON CONFLICT (name) DO UPDATE SET value = '', updated_at = now()")
        .execute(&mut *tx).await
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
                "INSERT INTO site_settings (name, value) VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
            ).bind(format!("module_{k}")).bind(val)
            .execute(&mut *tx).await;
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
                    "INSERT INTO section_kinds (kind, label, sort) VALUES ($1, $1, 999) ON CONFLICT (kind) DO NOTHING",
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
                    sqlx::query("INSERT INTO section_dict (kind, name, sort) VALUES ($1, $2, $3)")
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

/// 站型切换 diff 预览（U2 §8.2 向导第二步）：返回 apply 将改动的键旧值→新值，
/// 不落库。站长确认后才走 apply。
#[derive(Deserialize)]
struct PackDiffBody {
    code: String,
}
#[post("/admin/site-type-packs/diff")]
async fn site_type_pack_diff(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PackDiffBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let pack: Option<SiteTypePack> = sqlx::query_as(
        "SELECT code, name, description, brand, categories, modules, sort FROM site_type_packs WHERE code = $1",
    ).bind(&body.code)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(pack) = pack else {
        return Err(DomainError::Validation("类型包不存在".into()));
    };
    // 当前值快照（site_type/site_name/module_*）
    let current: Vec<(String, String)> =
        sqlx::query_as("SELECT name, value FROM site_settings WHERE name IN ('site_type','site_name') OR name LIKE 'module\\_%'")
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
    let cur = std::collections::HashMap::<String, String>::from_iter(current);
    let mut changes: Vec<serde_json::Value> = Vec::new();
    let mut push = |key: &str, old: Option<&String>, new: &str| {
        let old_v = old.map(|s| s.as_str()).unwrap_or("(未设置)");
        if old_v != new {
            changes.push(
                serde_json::json!({ "key": key, "old": old_v, "new": new }),
            );
        }
    };
    push("site_type", cur.get("site_type"), &pack.code);
    push("site_name", cur.get("site_name"), &pack.brand);
    if let Some(mods) = pack.modules.as_object() {
        for (k, v) in mods {
            let setting = format!("module_{k}");
            let new = if v.as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            push(&setting, cur.get(&setting), new);
        }
    }
    Ok(ok(serde_json::json!({
        "pack": pack.code,
        "name": pack.name,
        "changes": changes,
        "unchanged_modules": cur.len().saturating_sub(changes.len()),
    })))
}

/// 自定义站型另存（U2 §7.3 / U5 分发）：读当前站点配置快照存为新包
/// （code 前缀 custom_），预置包只读——满足「第 12 种站型」。
#[derive(Deserialize)]
struct PackSaveBody {
    code: String,
    name: String,
}
#[post("/admin/site-type-packs/save")]
async fn site_type_pack_save(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<PackSaveBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    let code = body.code.trim().to_lowercase();
    if !code.starts_with("custom_")
        || code.len() > 40
        || !code.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(DomainError::Validation(
            "code 需以 custom_ 开头，仅小写字母/数字/下划线，≤40 字符".into(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(DomainError::Validation("name 不能为空".into()));
    }
    // 当前配置快照：分类 / 模块开关 / 站名
    let cats: Vec<serde_json::Value> = sqlx::query_as::<_, (i32, String)>(
        "SELECT id, name FROM categories ORDER BY id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .into_iter()
    .map(|(id, name)| serde_json::json!({"id": id, "name": name}))
    .collect();
    let mods_rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name LIKE 'module\\_%'",
    )
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    let modules: serde_json::Map<String, serde_json::Value> = mods_rows
        .into_iter()
        .filter_map(|(name, value)| {
            name.strip_prefix("module_")
                .map(|k| (k.to_string(), serde_json::json!(value == "yes")))
        })
        .collect();
    let brand: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'site_name'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_default();
    // 自定义包的默认标语 = 快照时刻的登录页标语（覆盖值优先，空回落当前站型包默认）
    let pack_default_tagline: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT p.tagline FROM site_type_packs p \
         JOIN site_settings s ON s.name = 'site_type' AND s.value = p.code",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .filter(|t| !t.trim().is_empty());
    let override_tagline: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'site_tagline'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v| v.trim().to_string())
    .filter(|v| !v.is_empty());
    let tagline = override_tagline
        .or(pack_default_tagline)
        .unwrap_or_default();
    let sort: i32 = sqlx::query_scalar(
        "SELECT COALESCE(max(sort), 100) + 1 FROM site_type_packs",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(101);
    sqlx::query(
        "INSERT INTO site_type_packs (code, name, description, brand, categories, modules, sort, tagline) \
         VALUES ($1, $2, '自定义站型（另存快照）', $3, $4::jsonb, $5::jsonb, $6, $7) \
         ON CONFLICT (code) DO UPDATE SET name = EXCLUDED.name, brand = EXCLUDED.brand, \
           categories = EXCLUDED.categories, modules = EXCLUDED.modules, tagline = EXCLUDED.tagline",
    )
    .bind(&code)
    .bind(body.name.trim())
    .bind(&brand)
    .bind(serde_json::Value::Array(cats).to_string())
    .bind(serde_json::Value::Object(modules).to_string())
    .bind(sort)
    .bind(&tagline)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "site_type_pack_save", None)
        .await;
    Ok(ok(serde_json::json!({ "saved": code })))
}

// ---- 捐赠中心（馒头 donate 口径：储值钱包 + 三区套餐 + VIP）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct DonatePlan {
    id: i32,
    plan_type: String,
    title: String,
    reward: Option<String>,
    #[sqlx(default)]
    price_usd: f64,
    sort: i32,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct DonateLedgerRow {
    id: i64,
    kind: String,
    #[sqlx(default)]
    amount_usd: f64,
    #[sqlx(default)]
    balance_after: f64,
    note: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// 捐赠中心总览：钱包余额 + VIP 状态 + 套餐 + 我的流水
#[get("/donate/state")]
async fn donate_state(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let (wallet, vip_until): (f64, Option<chrono::DateTime<chrono::Utc>>) =
        sqlx::query_as(
            "SELECT wallet_usd::float8, vip_until FROM users WHERE id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let plans: Vec<DonatePlan> = sqlx::query_as(
        "SELECT id, plan_type, title, reward, price_usd::float8, sort FROM donation_plans WHERE enabled ORDER BY sort, id",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let ledger: Vec<DonateLedgerRow> = sqlx::query_as(
        "SELECT id, kind, amount_usd::float8, balance_after::float8, note, created_at          FROM donation_ledger WHERE user_id = $1 ORDER BY id DESC LIMIT 30",
    ).bind(auth.id)
    .fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        "wallet_usd": wallet,
        "vip_until": vip_until,
        "plans": plans,
        "ledger": ledger,
        // U4 §12.1：通道可用性（provider=epay 且凭证齐全；FLUX_DEMO 演示模式恒开）
        "payment_enabled": crate::payment::gateway_config(&state).await.available()
            || std::env::var("FLUX_DEMO").unwrap_or_default() == "1",
    })))
}

#[derive(Deserialize)]
struct TopupBody {
    amount_usd: f64,
    #[serde(default)]
    channel: String, // alipay / wechat（Dev 环境仅记录）
}

/// 充值（Dev 无支付网关：直接入账，模拟支付成功）
#[post("/donate/topup")]
async fn donate_topup(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TopupBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(10.0..=66.0).contains(&body.amount_usd) {
        return Err(DomainError::Validation("单笔 10 ~ 66 USD".into()));
    }
    if !["alipay", "wechat"].contains(&body.channel.as_str()) {
        return Err(DomainError::Validation(
            "支付方式需为 alipay/wechat".into(),
        ));
    }
    // U4 §12.1：真实订单流——按 site_settings 构造网关；未配置（none/缺凭证）拒单。
    // FLUX_DEMO=1 演示环境保留旧模拟充值（演示数据需要有钱包入账可看）。
    let gw = crate::payment::gateway_config(&state).await;
    if !gw.available() {
        if std::env::var("FLUX_DEMO").unwrap_or_default() == "1" {
            return crate::payment::demo_topup(
                &state,
                auth.id,
                body.amount_usd,
                &body.channel,
            )
            .await
            .map(|v| ok(v));
        }
        return Err(DomainError::Validation(
            "捐赠通道未开放（站长未配置支付网关）".into(),
        ));
    }
    // 建订单 + 返回跳转 URL（入账只发生在验签通过的回调，此端点不动钱包）
    let order_no = crate::payment::new_order_no(auth.id);
    sqlx::query(
        "INSERT INTO payment_orders (order_no, user_id, amount_usd, channel) VALUES ($1, $2, $3, $4)",
    )
    .bind(&order_no)
    .bind(auth.id)
    .bind(body.amount_usd)
    .bind(&body.channel)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let provider = crate::payment::provider_from(&gw);
    let base = std::env::var("PUBLIC_SITE_URL")
        .unwrap_or_else(|_| "http://localhost:3000".into());
    let url = provider.pay_url(
        &order_no,
        &format!("{:.2}", body.amount_usd),
        &body.channel,
        "站点捐赠",
        &format!("{base}/donate?order={order_no}"),
        &format!("{base}/api/v1/donate/notify"),
    );
    state
        .repo
        .audit(Some(auth.id), "donate_topup_order", None)
        .await;
    Ok(ok(
        serde_json::json!({ "order_no": order_no, "pay_url": url }),
    ))
}

/// 捐赠订单状态查询（前端支付回跳后轮询）
#[get("/donate/order-status")]
async fn donate_order_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let Some(order_no) = q.get("order_no") else {
        return Err(DomainError::Validation("缺少 order_no".into()));
    };
    let st: Option<String> = sqlx::query_scalar(
        "SELECT status FROM payment_orders WHERE order_no = $1 AND user_id = $2",
    )
    .bind(order_no)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "order_no": order_no, "status": st }),
    ))
}

/// 支付网关异步回调（GET，易支付口径）：验签 → 幂等入账 → 纯文本 "success"
/// （网关要求响应 success 字面量，不走统一信封）
#[get("/donate/notify")]
async fn donate_notify(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> HttpResponse {
    let _ = req;
    match crate::payment::settle_notify(&state, &q.into_inner()).await {
        Ok(_) => HttpResponse::Ok().body("success"),
        Err(e) => {
            tracing::warn!(?e, "donate notify rejected");
            HttpResponse::Ok().body("fail")
        }
    }
}

/// 旧模拟充值逻辑（FLUX_DEMO=1 专用）：移入 payment.rs 兄弟函数，保留演示站能力
#[allow(dead_code)]
mod removed_legacy_simulated_topup {
    // 历史实现已由 payment::demo_topup 替代（编译期占位，防误回滚参考）
}

#[derive(Deserialize)]
struct OrderBody {
    plan_id: i32,
}

/// 用余额订购套餐（上传量 / 片单额度 / VIP）
#[post("/donate/order")]
async fn donate_order(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OrderBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let plan: Option<(String, String, Option<String>, f64)> = sqlx::query_as(
        "SELECT plan_type, title, reward, price_usd::float8 FROM donation_plans WHERE id = $1 AND enabled",
    ).bind(body.plan_id)
    .fetch_optional(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((plan_type, title, reward, price)) = plan else {
        return Err(DomainError::NotFound(body.plan_id as i64));
    };
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let balance: Option<f64> = sqlx::query_scalar(
        "UPDATE users SET wallet_usd = wallet_usd - $2 WHERE id = $1 AND wallet_usd >= $2 RETURNING wallet_usd::float8",
    ).bind(auth.id).bind(price)
    .fetch_optional(&mut *tx).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(balance) = balance else {
        return Err(DomainError::Validation(format!(
            "余额不足，还差 {:.2} USD",
            price
        )));
    };
    // 套餐生效
    match plan_type.as_str() {
        "upload" => {
            // 「100 GB 上传量」/「500 GB 上传量」
            let gb: i64 = title
                .split_whitespace()
                .next()
                .and_then(|w| w.parse().ok())
                .unwrap_or(0);
            // 必须落差额流水（P1）：快照权威在 traffic_ledger，reconcile_snapshots 每 6h
            // 把 users.uploaded 重算为 sum(delta_up)——只 UPDATE 快照不落流水，付费购买的
            // 上传量会在 6h 内被静默抹掉。对照 shop upload_credit 的双写。
            sqlx::query(
                "INSERT INTO traffic_ledger (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, now())",
            )
            .bind(auth.id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(auth.id)
            .bind(gb * 1024 * 1024 * 1024)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "quota" => {
            sqlx::query(
                "UPDATE users SET quota_extra = quota_extra + 10 WHERE id = $1",
            )
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        "vip" => {
            let days: i64 = if title.contains("终身") {
                36500
            } else if title.contains("180") {
                180
            } else {
                30
            };
            sqlx::query(
                "UPDATE users SET vip_until = GREATEST(COALESCE(vip_until, now()), now()) + make_interval(days => $2::int) WHERE id = $1",
            ).bind(auth.id).bind(days)
            .execute(&mut *tx).await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
        _ => {}
    }
    // 附赠邀请 1（invite_quota 是 (user_id, period) 结构：当天无行则插入 used=0）
    if reward.as_deref().unwrap_or("").contains("邀请") {
        sqlx::query(
            "INSERT INTO invite_quota (user_id, period, used) VALUES ($1, current_date, 0)              ON CONFLICT (user_id, period) DO NOTHING",
        )
        .bind(auth.id)
        .execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO donation_ledger (user_id, kind, amount_usd, balance_after, plan_id, note) VALUES ($1, 'order', -$2, $3, $4, $5)",
    ).bind(auth.id).bind(price).bind(balance).bind(body.plan_id).bind(&title)
    .execute(&mut *tx).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "donate_order", Some(body.plan_id as i64))
        .await;
    Ok(ok(
        serde_json::json!({ "plan": title, "wallet_usd": balance }),
    ))
}

// ---- 批量邮件（massmail）----

#[derive(Deserialize)]
struct MassMailBody {
    subject: String,
    body: String,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct MassMailRow {
    id: i32,
    subject: String,
    recipients: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    sender: Option<String>,
}

#[get("/admin/massmail")]
async fn massmail_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL)
        .await?;
    let rows: Vec<MassMailRow> = sqlx::query_as(
        "SELECT m.id, m.subject, m.recipients, m.created_at, u.username AS sender \
         FROM mass_mails m LEFT JOIN users u ON u.id = m.sent_by ORDER BY m.id DESC LIMIT 50",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[post("/admin/massmail")]
async fn massmail_send(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<MassMailBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::MASSMAIL)
        .await?;
    if body.subject.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("主题和正文不能为空".into()));
    }
    let users: i64 =
        sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO mass_mails (subject, body, sent_by, recipients) VALUES ($1,$2,$3,$4) RETURNING id",
    ).bind(body.subject.trim()).bind(&body.body).bind(auth.id).bind(users as i32)
    .fetch_one(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "massmail_send", None).await;
    // 审计修复（P1 永不投递）：注释宣称 worker 投递，但 worker 无任何 mass_mails 消费——
    // 邮件永不发出且响应报 queued 误导操作者。现在当场投递（复用 build_smtp）：
    // SMTP 配置时逐户发送并回填实发数；未配置时（开发态）明确返回 delivered=0 与
    // 原因，不再谎报入队。
    let smtp = std::env::var("SMTP_URL").unwrap_or_default();
    let from = std::env::var("SMTP_FROM")
        .unwrap_or_else(|_| "no-reply@fluxtorrent.local".into());
    let mut delivered: i64 = 0;
    let mut mail_err: Option<String> = None;
    if smtp.is_empty() {
        mail_err = Some("SMTP_URL 未配置（开发态：邮件未投递，仅留档）".into());
    } else {
        match crate::gaps_http::build_smtp(&smtp) {
            Ok(mailer) => {
                let emails: Vec<String> = sqlx::query_scalar(
                    "SELECT email FROM users WHERE status < 2 AND email IS NOT NULL",
                )
                .fetch_all(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                for to in &emails {
                    let msg = lettre::Message::builder()
                        .from(from.parse().map_err(
                            |e: lettre::address::AddressError| {
                                DomainError::Internal(e.into())
                            },
                        )?)
                        .to(to.parse().map_err(
                            |e: lettre::address::AddressError| {
                                DomainError::Internal(e.into())
                            },
                        )?)
                        .subject(body.subject.trim())
                        .body(body.body.clone())
                        .map_err(|e| DomainError::Internal(e.into()))?;
                    use lettre::AsyncTransport;
                    match mailer.send(msg).await {
                        Ok(_) => delivered += 1,
                        Err(e) => {
                            mail_err =
                                Some(format!("第 {delivered} 封后失败：{e}"));
                            break;
                        }
                    }
                }
            }
            Err(e) => mail_err = Some(format!("SMTP 构建失败：{e}")),
        }
    }
    if delivered > 0 {
        let _ =
            sqlx::query("UPDATE mass_mails SET recipients = $2 WHERE id = $1")
                .bind(id)
                .bind(delivered as i32)
                .execute(&state.repo.db)
                .await;
    }
    Ok(ok(serde_json::json!({
        "id": id, "delivered": delivered,
        "queued": 0,
        "note": mail_err.unwrap_or_else(|| "已全部投递".into()),
    })))
}
