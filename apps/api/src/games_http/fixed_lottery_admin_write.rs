//! 固定抽签管理口·写侧（0520 拆件守 300 行门禁）：创建草稿。
//! 读侧与状态口在 fixed_lottery_admin_http.rs；本文件只承载
//! CampaignBody/PrizeBody + gen_code + create + parse_ts。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::authz;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::fixed_lottery_admin as rpa;
use crate::state::AppState;

/// 奖励类型白名单，与 0520 迁移的 CHECK 约束同源。
const KNOWN_KINDS: &[&str] = &[
    "spark",
    "invite",
    "item",
    "medal",
    "upload",
    "vip",
    "ad_free",
    "rainbow_id",
    "rename_card",
    "custom",
];

#[derive(Deserialize)]
pub(super) struct CampaignBody {
    pub title: String,
    #[serde(default)]
    pub intro: String,
    #[serde(default)]
    pub trigger_type: String,
    #[serde(default)]
    pub scheduled_draw_at: Option<String>,
    #[serde(default)]
    pub threshold_count: Option<i32>,
    #[serde(default)]
    pub fallback_draw_at: Option<String>,
    #[serde(default)]
    pub starts_at: Option<String>,
    #[serde(default)]
    pub ticket_price: i64,
    #[serde(default)]
    pub eligibility: serde_json::Value,
    #[serde(default)]
    pub claim_default_days: Option<i32>,
    #[serde(default)]
    pub prizes: Vec<PrizeBody>,
}

#[derive(Deserialize)]
pub(super) struct PrizeBody {
    pub name: String,
    #[serde(default)]
    pub quantity: i32,
    pub reward_kind: String,
    #[serde(default)]
    pub reward_config: serde_json::Value,
}

/// 奖励配置按类型校验，错误在创建口暴露，别拖到履约 5 次重试后。
fn validate_reward_config(p: &PrizeBody) -> DomainResult<()> {
    let cfg = &p.reward_config;
    match p.reward_kind.as_str() {
        "spark" => {
            if cfg.get("amount").and_then(|v| v.as_i64()).unwrap_or(0) <= 0 {
                return Err(DomainError::Validation(
                    "火花奖励必须配置有效金额".into(),
                ));
            }
        }
        "medal" => {
            if cfg.get("medal_id").and_then(|v| v.as_i64()).unwrap_or(0) <= 0 {
                return Err(DomainError::Validation(
                    "勋章奖励必须配置有效 medal_id".into(),
                ));
            }
        }
        "item" => {
            if cfg
                .get("item_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Err(DomainError::Validation(
                    "物品奖励必须配置 item_key".into(),
                ));
            }
        }
        "upload" => {
            if cfg.get("gb").and_then(|v| v.as_i64()).unwrap_or(0) <= 0 {
                return Err(DomainError::Validation(
                    "上传量奖励必须配置有效 gb".into(),
                ));
            }
        }
        "vip" | "ad_free" => {
            if cfg.get("days").and_then(|v| v.as_i64()).unwrap_or(7) <= 0 {
                return Err(DomainError::Validation("天数奖励配置非法".into()));
            }
        }
        "rainbow_id" => {
            if cfg.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0) <= 0 {
                return Err(DomainError::Validation(
                    "彩虹ID奖励必须配置有效 item_id".into(),
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn gen_code() -> String {
    use rand::Rng;
    const ALPHA: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    (0..8)
        .map(|_| ALPHA[rand::thread_rng().gen_range(0..ALPHA.len())] as char)
        .collect()
}

/// 创建草稿（含奖项覆盖写）。参与码生成失败重试一次。
#[post("/admin/lottery/campaigns")]
pub(super) async fn admin_lottery_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<CampaignBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    authz::require_perm(&state, &auth, authz::perm::USER_ADJUST).await?;
    let b = body.into_inner();
    if b.title.trim().is_empty() {
        return Err(DomainError::Validation("活动标题不能为空".into()));
    }
    if b.prizes.is_empty() || b.prizes.len() > 5 {
        return Err(DomainError::Validation("奖项数量须在 1-5 之间".into()));
    }
    for p in &b.prizes {
        if p.quantity < 1 || p.quantity > 1000 {
            return Err(DomainError::Validation(
                "奖项名额须在 1-1000 之间".into(),
            ));
        }
        if p.name.trim().is_empty() {
            return Err(DomainError::Validation("奖项名称不能为空".into()));
        }
        if !KNOWN_KINDS.contains(&p.reward_kind.as_str()) {
            return Err(DomainError::Validation("奖励类型非法".into()));
        }
        validate_reward_config(p)?;
    }
    match b.trigger_type.as_str() {
        "scheduled" => {
            if b.scheduled_draw_at.is_none() {
                return Err(DomainError::Validation(
                    "定时开奖必须设置开奖时间".into(),
                ));
            }
        }
        "threshold" => {
            if b.threshold_count.unwrap_or(0) < 1 {
                return Err(DomainError::Validation(
                    "人数达标必须设置有效阈值".into(),
                ));
            }
            if b.fallback_draw_at.is_none() {
                return Err(DomainError::Validation(
                    "人数达标必须设置保底开奖时间".into(),
                ));
            }
        }
        _ => {
            return Err(DomainError::Validation("开奖方式非法".into()));
        }
    }
    let mut code = gen_code();
    let mut attempt = 0;
    loop {
        let dup = rpa::code_exists(&state.repo, &code).await?;
        if !dup {
            break;
        }
        attempt += 1;
        if attempt > 3 {
            return Err(DomainError::Internal(anyhow::anyhow!(
                "参与码生成冲突"
            )));
        }
        code = gen_code();
    }
    let mut tx = state
        .repo
        .db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let prize_rows: Vec<(String, i32, String, serde_json::Value)> = b
        .prizes
        .iter()
        .map(|p| {
            (
                p.name.trim().to_string(),
                p.quantity,
                p.reward_kind.clone(),
                p.reward_config.clone(),
            )
        })
        .collect();
    let cid = rpa::create_campaign_tx(
        &mut tx,
        &code,
        b.title.trim(),
        &b.intro,
        &b.trigger_type,
        parse_ts(b.starts_at.as_deref())?,
        parse_ts(b.scheduled_draw_at.as_deref())?,
        b.threshold_count,
        parse_ts(b.fallback_draw_at.as_deref())?,
        b.ticket_price.clamp(0, 1_000_000),
        &b.eligibility,
        Some(b.claim_default_days.unwrap_or(14).clamp(1, 365)),
        auth.id,
        &prize_rows,
    )
    .await?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "lottery.create", Some(cid))
        .await;
    Ok(ok(serde_json::json!({"id": cid, "code": code})))
}

fn parse_ts(
    s: Option<&str>,
) -> DomainResult<Option<chrono::DateTime<chrono::Utc>>> {
    match s {
        None | Some("") => Ok(None),
        Some(v) => chrono::DateTime::parse_from_rfc3339(v)
            .map(|d| Some(d.with_timezone(&chrono::Utc)))
            .map_err(|_| DomainError::Validation("时间格式非法".into())),
    }
}
