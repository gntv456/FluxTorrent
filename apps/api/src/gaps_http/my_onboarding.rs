//! GET /me/onboarding —— 新手清单：每一步都由数据库事实判定。
//!
//! 与后台 onboarding 预设（ops_http/onboarding.rs）不同：那是站长选
//! 运营风格写设定，这里是**会员本人**的完成度。所有 done 都来自库里的
//! 真实痕迹（验证时间、passkey 行、snatches 计数），不接受前端自报
//! 点击，也不为此新增"已读"表。
//!
//! 邮件一步只在站点确实要求验证（registration_mode=email_verify）时
//! 出现：开放/邀请模式下新号的 email_verified_at 本来就是 NULL，若照
//! 列会让这些站点的会员永远差一步、清单永远打不满。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::auth::{onboarding_facts, OnboardingFacts};
use crate::state::AppState;

#[derive(serde::Serialize)]
struct Step {
    key: &'static str,
    done: bool,
    href: &'static str,
}

fn build_steps(f: &OnboardingFacts) -> Vec<Step> {
    let mut out = Vec::with_capacity(7);
    if f.email_verify_mode {
        out.push(Step {
            key: "email",
            done: f.email_verified,
            href: "/my?tab=personal",
        });
    }
    out.push(Step {
        key: "profile",
        done: f.profile,
        href: "/my?tab=personal",
    });
    out.push(Step {
        key: "first_download",
        done: f.first_download,
        href: "/torrents",
    });
    out.push(Step {
        key: "first_seed",
        done: f.first_seed,
        href: "/myhr",
    });
    out.push(Step {
        key: "passkey",
        done: f.passkey,
        href: "/my?tab=security",
    });
    out.push(Step {
        key: "twofa",
        done: f.totp_enabled,
        href: "/my?tab=security",
    });
    out.push(Step {
        key: "rss",
        done: f.rss_token,
        href: "/getrss",
    });
    out
}

#[get("/me/onboarding")]
pub async fn my_onboarding(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let facts = onboarding_facts(&state.repo, auth.id)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .ok_or(DomainError::NotFound(auth.id))?;
    let steps = build_steps(&facts);
    let done = steps.iter().filter(|s| s.done).count();
    let total = steps.len();
    Ok(ok(serde_json::json!({
        "steps": steps,
        "done": done,
        "total": total,
        "completed": done == total,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> OnboardingFacts {
        OnboardingFacts {
            email_verified: false,
            email_verify_mode: false,
            totp_enabled: false,
            passkey: false,
            rss_token: false,
            profile: false,
            first_download: false,
            first_seed: false,
        }
    }

    #[test]
    fn email_step_only_when_site_requires_verification() {
        let f = facts();
        let keys: Vec<_> = build_steps(&f).iter().map(|s| s.key).collect();
        assert_eq!(keys.len(), 6);
        assert!(!keys.contains(&"email"));

        let mut f2 = facts();
        f2.email_verify_mode = true;
        let keys: Vec<_> = build_steps(&f2).iter().map(|s| s.key).collect();
        assert_eq!(keys.len(), 7);
        assert_eq!(keys[0], "email");
    }

    #[test]
    fn every_done_flag_comes_from_facts() {
        let mut f = facts();
        f.email_verify_mode = true;
        f.email_verified = true;
        f.profile = true;
        f.first_download = true;
        f.first_seed = true;
        f.passkey = true;
        f.totp_enabled = true;
        f.rss_token = true;
        let steps = build_steps(&f);
        assert_eq!(steps.len(), 7);
        assert!(steps.iter().all(|s| s.done));
    }

    #[test]
    fn partial_progress_is_not_complete() {
        let mut f = facts();
        f.profile = true;
        let steps = build_steps(&f);
        let done = steps.iter().filter(|s| s.done).count();
        assert_eq!(done, 1);
        assert_eq!(steps.len(), 6);
        assert_ne!(done, steps.len());
    }
}
