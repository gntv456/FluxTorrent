//! 安装向导（U3 §8.3）：冷启动检测 → 三步向导 → setup_done 置位。
//!
//! 未完成时封锁一切业务 API（防裸奔站被扫），仅放行：
//! /setup*、/health、/auth/login（站长可能需要先登录）、静态兼容探针 /compat/meta。
//! 向导完成动作：purge demo → 应用站型包 extras → 建管理员（如未建）→ 置 setup_done。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

mod status;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::ops_http::OnboardingApplyBodyRepr;
use crate::state::AppState;
use status::setup_status;

/// 业务 API 封锁中间件（module_gate 之前判断）：
/// setup_done 未置位且路径不在白名单 → 403 引导向 /setup。
pub async fn setup_gate_mw(
    req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody>,
) -> actix_web::Result<
    actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
> {
    let path = req.path();
    const ALLOW: &[&str] = &[
        "/api/v1/setup",
        "/api/v1/setup/status",
        "/api/v1/health",
        "/api/v1/auth/login",
        "/api/v1/auth/logout",
        // 装机自救通道：0017 引导的 root 带 must_reset_password=true，而 auth_infra 的
        // 强制改密闸门只放行 /me、/me/password*、/auth/logout。这里若不放行改密，
        // 首启就死锁：改密被装机门拦 → 完不成向导（向导要鉴权）→ 站点永远装不完。
        "/api/v1/me/password",
        "/api/v1/compat/meta",
        "/api/v1/metrics",
    ];
    if !path.starts_with("/api/v1") || ALLOW.iter().any(|p| path.starts_with(p))
    {
        return next.call(req).await;
    }
    let state = req
        .app_data::<web::Data<std::sync::Arc<AppState>>>()
        .cloned()
        .expect("AppState registered");
    let done: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'setup_done'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    if done == "done" {
        return next.call(req).await;
    }
    // FLUX_DEMO=1 演示环境跳过封锁（演示站要能直接看）
    if std::env::var("FLUX_DEMO").unwrap_or_default() == "1" {
        return next.call(req).await;
    }
    Err(actix_web::Error::from(DomainError::Validation(
        "站点尚未完成安装向导，请先访问 /setup".into(),
    )))
}

#[derive(Deserialize)]
struct SetupFinishBody {
    /// 所选站型包 code（空 = 保持当前）
    #[serde(default)]
    pack: String,
    /// 站名（空 = 不改）
    #[serde(default)]
    site_name: String,
    /// 公网 tracker announce 地址（空 = 不改；P0-2.1：向导内采集，
    /// 拦住「装完仍是 127.0.0.1、种子的 tracker 永远收不到请求」的死链）
    #[serde(default)]
    announce_url: String,
    /// 新手运营模板（C4，0226）：strict | lenient | 空（跳过 = 维持现状）
    #[serde(default)]
    onboarding_preset: String,
    /// 玩法合规确认（§11.3：必须显式 true 才允许完成，留审计）
    games_compliance_ack: bool,
}

/// 完成向导（sysop）：purge demo → 应用包 → 置位。幂等可重入。
#[post("/setup")]
async fn setup_finish(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SetupFinishBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SITEPACKS_MANAGE,
    )
    .await?;
    if !body.games_compliance_ack {
        return Err(DomainError::Validation(
            "需确认已了解娱乐玩法（机会类游戏）的本地合规要求".into(),
        ));
    }
    // 1) demo 清理（幂等：重复执行零行）
    let purged: Vec<(String, i64)> =
        sqlx::query_as("SELECT kind, removed FROM purge_demo_data()")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 2) 站型完整应用（二审 G6 修复：与 /admin/site-type-packs/apply 同一条
    //    pack_core::apply_pack_full 链路——分类/module_*/sections/tags/extras/
    //    字幕口径全部落地；空库无种子，replace 模式重建分类安全）
    let mut extras_applied: Vec<(String, i64)> = Vec::new();
    let mut applied_pack: Option<String> = None;
    if !body.pack.is_empty() {
        let pack: Option<crate::staff_http::setup_bridge::PackRef> =
            sqlx::query_as(
                "SELECT code, name, description, brand, categories, modules, \
                 sort, sections, tags, tagline, subtitle_kind \
                 FROM site_type_packs WHERE code = $1",
            )
            .bind(&body.pack)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let Some(pack) = pack else {
            return Err(DomainError::Validation("站型包不存在".into()));
        };
        let (_cats, extras) = crate::staff_http::setup_bridge::apply_pack_full(
            &state.repo.db,
            &pack,
            "replace",
        )
        .await?;
        extras_applied = extras;
        applied_pack = Some(body.pack.clone());
    }
    // 3) 站名（可选；apply 已写过包 brand，这里站长显式输入优先）
    if !body.site_name.trim().is_empty() {
        // 绑参必须从 $1 起：写成 $2 而只 bind 一个值时，Postgres 会推出一个
        // 类型未定的 $1、prepare 直接失败，再被 let _ 吞掉 → 站名静默不落库
        sqlx::query(
            "INSERT INTO site_settings (name, value) VALUES ('site_name', $1) \
             ON CONFLICT (name) DO UPDATE SET value = $1, updated_at = now()",
        )
        .bind(body.site_name.trim())
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // 3b) announce_url（可选但强烈引导；P0-2.1）。仅拒绝「仍是本地回环」的
    // 显式填入——不填视为「保持现状」（幂等重入不炸老站），留给 checklist 兜底警示。
    if !body.announce_url.trim().is_empty() {
        let a = body.announce_url.trim();
        if a.contains("127.0.0.1") || a.contains("localhost") {
            return Err(DomainError::Validation(
                "announce 地址不能是 127.0.0.1/localhost——请填写你站点的公网地址 \
                 （其他用户下载种子后通过它连接你的 Tracker）"
                    .into(),
            ));
        }
        // 同上：$1 起绑参，且失败不再吞（写不进去要当场报错，不能静默回退）
        sqlx::query(
            "INSERT INTO site_settings (name, value) VALUES ('announce_url', \
             $1) ON CONFLICT (name) DO UPDATE SET value = $1, updated_at = \
             now()",
        )
        .bind(a)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // 3c) 新手运营模板（C4，0226）：向导内可选 strict/lenient——复用
    //     onboarding_apply 的参数簇写入（权限已由本端点的 SITEPACKS_MANAGE
    //     覆盖；空 = 跳过维持现状）。
    if !body.onboarding_preset.is_empty() {
        let preset = body.onboarding_preset.as_str();
        if !matches!(preset, "strict" | "lenient") {
            return Err(DomainError::Validation(
                "onboarding_preset 仅支持 strict / lenient".into(),
            ));
        }
        let inner = OnboardingApplyBodyRepr {
            preset: preset.to_string(),
        };
        let _ =
            crate::ops_http::setup_apply_onboarding(&state, auth.id, &inner)
                .await?;
    }
    // 4) 置位（幂等）
    sqlx::query(
        "INSERT INTO site_settings (name, value) VALUES ('setup_done', 'done') \
         ON CONFLICT (name) DO UPDATE SET value = 'done', updated_at = now()",
    )
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.module_flags.invalidate().await;
    crate::cfgver::bump(state.get_ref(), "modules").await;
    // 5) 首个邀请码（P0-2.2 注册死锁）：注册模式为 invite_only 且站内尚无
    //    可用码时，自动发一枚给完成向导的管理员——否则新站长无法产生第二个
    //    用户（自己没配额、后台路径也不知道）。幂等：已有未用码不重复发。
    let mut first_invite: Option<String> = None;
    let (reg_mode, unused): (String, i64) = sqlx::query_as(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'registration_mode'), 'invite_only'), \
         COALESCE((SELECT count(*) FROM invites WHERE status = 0 AND \
         expires_at > now()), 0)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if reg_mode == "invite_only" && unused == 0 {
        let code = crate::domain::new_invite_code();
        let expires = chrono::Utc::now() + chrono::Duration::hours(72);
        let _ = state.repo.issue_invite(auth.id, &code, expires).await?;
        first_invite = Some(code);
    }
    state.repo.audit(Some(auth.id), "setup_finish", None).await;
    Ok(ok(serde_json::json!({
        "done": true,
        "purged": purged,
        "pack": applied_pack,
        "extras": extras_applied,
        "compliance_ack": true,
        "first_invite": first_invite,
    })))
}

pub fn mount_setup(scope: actix_web::Scope) -> actix_web::Scope {
    scope.service(setup_status).service(setup_finish)
}
