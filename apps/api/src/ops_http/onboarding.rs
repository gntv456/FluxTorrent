//! C4 新手运营双模板（0226）：选择态读取 + 一键应用参数簇。
//!
//! 两种已被竞品源码级验证的新手哲学（NexusPHP 考核淘汰制 / UNIT3D 缓冲宽进制）
//! 收敛为站长的一键选项——「不偏向任何 PT 类型」延伸到运营风格层。
//!
//! 应用动作只写设定键与考核开关，**不动任何用户数据**；重复应用幂等。
//! 消费点全部复用既有链路：
//!   · hr_hours / hr_violation_limit / hr_warn / hr_prewarn_hours → worker hr.rs
//!   · exam auto_assign + exam_onboard_days → worker task_jobs.rs
//!   · class_rules.demotable → worker class_adj.rs（低保户降级开关）
//!   · initial_upload_gb → register.rs（新用户初始上传量）

use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 两套模板的参数簇（值来自竞品源码级调研的典型口径，站长之后可逐项再调）
const STRICT: &[(&str, &str)] = &[
    // 考核淘汰制：H&R 从严 + 考核开 + 低保户可降级 + 无初始缓冲
    ("hr_hours", "96"),
    ("hr_violation_limit", "3"),
    ("hr_prewarn_hours", "24"),
    ("initial_upload_gb", "0"),
];
const LENIENT: &[(&str, &str)] = &[
    // 缓冲宽进制：初始 50GB 缓冲 + H&R 宽松 + 预警前置 + 无强制考核
    ("hr_hours", "168"),
    ("hr_violation_limit", "5"),
    ("hr_prewarn_hours", "48"),
    ("initial_upload_gb", "50"),
];

async fn set_kv(db: &sqlx::PgPool, name: &str, value: &str) {
    // 键与 settings_meta 行都由 0226 预建（个别是既有键），此处只写值
    sqlx::query(
        "INSERT INTO site_settings (name, value) VALUES ($1, $2) \
         ON CONFLICT (name) DO UPDATE SET value = $2, updated_at = now()",
    )
    .bind(name)
    .bind(value)
    .execute(db)
    .await
    .expect("write setting");
}

/// 当前模板态 + 参数簇现值（后台「新手运营模板」页数据源）
#[get("/admin/onboarding")]
pub(super) async fn onboarding_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let keys = [
        "onboarding_preset",
        "hr_hours",
        "hr_violation_limit",
        "hr_prewarn_hours",
        "initial_upload_gb",
        "exam_onboard_days",
    ];
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name = ANY($1)",
    )
    .bind(&keys)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut kv: std::collections::BTreeMap<String, String> =
        rows.into_iter().collect();
    kv.entry("onboarding_preset".into())
        .or_insert_with(|| "none".into());
    // 考核派发现状（onboard 类任务是否 auto_assign）
    let exam_on: bool = sqlx::query_scalar(
        "SELECT COALESCE(bool_or(auto_assign), false) FROM tasks \
         WHERE kind = 'onboard'",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    Ok(ok(serde_json::json!({
        "current": kv.remove("onboarding_preset").unwrap_or_default(),
        "exam_auto_assign": exam_on,
        "params": kv,
    })))
}

#[derive(Deserialize)]
pub struct OnboardingApplyBodyRepr {
    /// none | strict | lenient
    pub preset: String,
}

/// 应用模板核心（供 setup 向导复用；不重复鉴权——调用方已持有更高权限）。
/// 参数簇写入 + 考核开关 + 低保户 demotable，幂等。
pub async fn setup_apply_onboarding(
    state: &web::Data<std::sync::Arc<AppState>>,
    actor_id: i64,
    body: &OnboardingApplyBodyRepr,
) -> DomainResult<Vec<String>> {
    let preset = body.preset.as_str();
    if !matches!(preset, "none" | "strict" | "lenient") {
        return Err(DomainError::Validation(
            "preset 仅支持 none / strict / lenient".into(),
        ));
    }
    let mut applied: Vec<String> = Vec::new();
    match preset {
        "strict" => {
            for (k, v) in STRICT {
                set_kv(&state.repo.db, k, v).await;
                applied.push((*k).to_string());
            }
            sqlx::query(
                "UPDATE tasks SET auto_assign = true WHERE kind = 'onboard'",
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE class_rules SET demotable = true WHERE class_id = 1",
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            applied.push("exam_auto_assign=on".into());
            applied.push("class_demotable=on".into());
        }
        "lenient" => {
            for (k, v) in LENIENT {
                set_kv(&state.repo.db, k, v).await;
                applied.push((*k).to_string());
            }
            sqlx::query(
                "UPDATE tasks SET auto_assign = false WHERE kind = 'onboard'",
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            sqlx::query(
                "UPDATE class_rules SET demotable = false WHERE class_id = 1",
            )
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            applied.push("exam_auto_assign=off".into());
            applied.push("class_demotable=off".into());
        }
        _ => {}
    }
    set_kv(&state.repo.db, "onboarding_preset", preset).await;
    state.module_flags.invalidate().await;
    crate::cfgver::bump(state.get_ref(), "settings").await;
    state
        .repo
        .audit(Some(actor_id), "onboarding_apply", None)
        .await;
    Ok(applied)
}

/// 应用模板（sysop）：写参数簇 + 考核开关 + 低保户 demotable，幂等。
#[post("/admin/onboarding")]
pub(super) async fn onboarding_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<OnboardingApplyBodyRepr>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let applied = setup_apply_onboarding(&state, auth.id, &body).await?;
    Ok(ok(serde_json::json!({
        "preset": body.preset,
        "applied": applied,
    })))
}
