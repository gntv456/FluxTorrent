//! GET /system/policies —— 站点公开口径的单一事实源（版本化）。
//!
//! 会员页与公开说明此前各读各的键、各写各的默认：H&R 的做种时长与
//! 期限在展示侧被写死过，而 worker 实际按种子政策与等级折扣执行，
//! 同一名会员能看到两种承诺。这里把「生效值」一次算好并给版本号，
//! 其余读口只需引用版本，站方改配置后可被跨页对账。
//!
//! 只公开**结果**：做种义务、预警与违规阈值、风险衰减半衰期。
//! 不公开反作弊权重/分档阈值，也不含任何用户数据。

use actix_web::{get, web, HttpResponse};

use crate::domain::policy::PolicySnapshot;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::repo::settings;
use crate::state::AppState;

#[derive(serde::Serialize)]
struct PolicyResp {
    #[serde(flatten)]
    policy: PolicySnapshot,
    /// 会员可读的风险说明页（本人分数、原因码与申诉入口）
    risk_explanation_path: &'static str,
    /// 抽奖公正性校验端点模板（commit-reveal，公开可复核）
    lottery_verification_path: &'static str,
    lottery_algorithm: &'static str,
}

#[get("/system/policies")]
pub async fn policy_public(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let policy = settings::policy_snapshot(&state.repo)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(PolicyResp {
        policy,
        risk_explanation_path: "/account-health",
        lottery_verification_path: "/api/v1/forums/lottery/{topic_id}/fairness",
        lottery_algorithm: "sha256-commit-reveal-v1",
    }))
}
