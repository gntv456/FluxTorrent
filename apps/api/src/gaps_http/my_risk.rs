//! GET /me/risk —— 会员可读的作弊风险解释（原因码 + 申诉入口）。
//!
//! 此前分数与证据只在 `/admin/*` 可见：会员突然被停下载权，界面上
//! 只有一句通用处置，没有任何可核对的原因，也没有指向申诉的路径。
//! 本端点把**本人**的分数、存活证据按原因码聚合后公开，并给出
//! 「现在能不能申诉、申诉到哪」。
//!
//! 披露边界：不返回 peer_ip（可关联他人）、不返回权重/阈值
//! （等于把判据交给对手）、不返回申诉正文（只回最近一条的结论摘要）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::repo::cheat_accuracy as risk_repo;
use crate::repo::settings;
use crate::state::AppState;

use super::cheat_appeals::MAX_OPEN;

/// 证据的 `agent` 前缀 → 稳定原因码（附可解析出的种子号）。
///
/// 与 worker `risk_score` 的家族划分对齐，但更细：`xreport:absurd`
/// 与 `login_ip_spread` 在打分里都落 default 家族，对用户解释时
/// 必须分开说，否则「你违反了默认规则」没有任何信息量。
fn classify(agent: &str, reason: &str) -> (&'static str, Option<i64>) {
    // 旧口径 'connectable' 已并入 ghost 家族（announce_main），存量行
    // 会一直留到证据被 resolve；UA 形态的 agent 是客户端规则拦截。
    if agent == "connectable" {
        ("ghost_seed", None)
    } else if reason.starts_with("客户端被禁止")
        || reason.starts_with("客户端不在允许列表")
    {
        ("client_blocked", None)
    } else if agent.starts_with("ghost:") {
        ("ghost_seed", None)
    } else if let Some(r) = agent.strip_prefix("speed:") {
        ("speed_over_ceiling", r.parse().ok())
    } else if let Some(r) = agent.strip_prefix("reset:") {
        ("counter_reset", r.parse().ok())
    } else if let Some(r) = agent.strip_prefix("corr:") {
        ("uncorroborated_upload", r.parse().ok())
    } else if let Some(r) = agent.strip_prefix("xreport_over:") {
        ("xreport_over", r.parse().ok())
    } else if let Some(r) = agent.strip_prefix("nearcap:") {
        ("near_cap_rhythm", r.parse().ok())
    } else if agent.starts_with("deal:") {
        ("self_deal", None)
    } else if let Some(r) = agent.strip_prefix("torrent:") {
        ("torrent_audit", r.parse().ok())
    } else {
        match agent {
            "xreport:absurd" => ("xreport_absurd", None),
            "down_under" => ("download_under_report", None),
            "bitthief" => ("bit_thief", None),
            "login_ip_spread" => ("login_ip_spread", None),
            _ => ("other", None),
        }
    }
}

#[derive(serde::Serialize)]
struct ReasonRow {
    code: &'static str,
    hits: i64,
    torrent_ids: Vec<i64>,
    first_seen: Option<chrono::DateTime<chrono::Utc>>,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
    /// 证据表里写给运营看的说明，原样给本人——比只有原因码可核对
    detail: String,
}

#[derive(Default)]
struct Agg {
    hits: i64,
    torrent_ids: Vec<i64>,
    first_seen: Option<chrono::DateTime<chrono::Utc>>,
    last_seen: Option<chrono::DateTime<chrono::Utc>>,
    detail: String,
}

fn aggregate(rows: &[risk_repo::MemberEvidence]) -> Vec<ReasonRow> {
    let mut map: std::collections::HashMap<&'static str, Agg> =
        std::collections::HashMap::new();
    for ev in rows {
        let (code, tid) = classify(&ev.agent, &ev.reason);
        let a = map.entry(code).or_default();
        a.hits += ev.hits;
        if let Some(t) = tid {
            if !a.torrent_ids.contains(&t) {
                a.torrent_ids.push(t);
            }
        }
        a.first_seen = Some(match a.first_seen {
            Some(v) => v.min(ev.first_seen),
            None => ev.first_seen,
        });
        a.last_seen = Some(match a.last_seen {
            Some(v) => v.max(ev.last_seen),
            None => ev.last_seen,
        });
        if a.detail.is_empty() {
            a.detail = ev.reason.clone();
        }
    }
    let mut out: Vec<ReasonRow> = map
        .into_iter()
        .map(|(code, a)| ReasonRow {
            code,
            hits: a.hits,
            torrent_ids: a.torrent_ids,
            first_seen: a.first_seen,
            last_seen: a.last_seen,
            detail: a.detail,
        })
        .collect();
    out.sort_by(|a, b| b.hits.cmp(&a.hits).then_with(|| a.code.cmp(b.code)));
    out
}

#[derive(serde::Serialize)]
struct MyRiskResp {
    score: i32,
    band: String,
    ruleset_ver: i32,
    updated_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 当前站点衰减半衰期（与 worker 打分同口径）
    decay_half_life_days: f64,
    reasons: Vec<ReasonRow>,
    open_case: Option<risk_repo::MemberCase>,
    latest_appeal: Option<risk_repo::AppealSummary>,
    /// 现在能否提交新申诉（有进行中案件、该案无未结申诉、未达上限）
    appeal_available: bool,
    /// 当前案件是否已有未结申诉（latest_appeal 可能属于别的案件）
    case_pending: bool,
    open_appeals: i64,
    appeal_limit: i64,
    appeal_path: &'static str,
}

#[get("/me/risk")]
pub async fn my_risk(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;
    let risk = risk_repo::member_risk(&state.repo, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let evidence = risk_repo::member_evidence(&state.repo, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let case = risk_repo::member_open_case(&state.repo, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let latest = risk_repo::latest_appeal(&state.repo, uid)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let case_id = case.as_ref().map(|c| c.id);
    let (open_count, case_pending) =
        risk_repo::open_flags(&state.repo, uid, case_id)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let decay = settings::policy_snapshot(&state.repo)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .risk_decay_days;
    let (score, band, ver, updated) = match risk {
        Some(r) => (r.score, r.band, r.ruleset_ver, Some(r.updated_at)),
        None => (0, "none".to_string(), 0, None),
    };
    Ok(ok(MyRiskResp {
        score,
        band,
        ruleset_ver: ver,
        updated_at: updated,
        decay_half_life_days: decay,
        reasons: aggregate(&evidence),
        appeal_available: case.is_some()
            && !case_pending
            && open_count < MAX_OPEN,
        case_pending,
        open_appeals: open_count,
        appeal_limit: MAX_OPEN,
        open_case: case,
        latest_appeal: latest,
        appeal_path: "/account-health#appeal",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ev(agent: &str, hits: i64, day: u32) -> risk_repo::MemberEvidence {
        let t = chrono::Utc
            .with_ymd_and_hms(2026, 10, day, 0, 0, 0)
            .unwrap();
        risk_repo::MemberEvidence {
            agent: agent.to_string(),
            reason: format!("{agent} 的说明"),
            hits,
            first_seen: t,
            last_seen: t,
        }
    }

    #[test]
    fn classify_covers_every_worker_agent() {
        assert_eq!(classify("ghost:abcd1234", "").0, "ghost_seed");
        assert_eq!(classify("speed:42", ""), ("speed_over_ceiling", Some(42)));
        assert_eq!(classify("reset:7", ""), ("counter_reset", Some(7)));
        assert_eq!(classify("corr:9", ""), ("uncorroborated_upload", Some(9)));
        assert_eq!(classify("xreport_over:11", ""), ("xreport_over", Some(11)));
        assert_eq!(classify("nearcap:3", ""), ("near_cap_rhythm", Some(3)));
        assert_eq!(classify("deal:deadbeef", "").0, "self_deal");
        assert_eq!(classify("torrent:5", ""), ("torrent_audit", Some(5)));
        assert_eq!(classify("xreport:absurd", "").0, "xreport_absurd");
        assert_eq!(classify("down_under", "").0, "download_under_report");
        assert_eq!(classify("bitthief", "").0, "bit_thief");
        assert_eq!(classify("login_ip_spread", "").0, "login_ip_spread");
        assert_eq!(classify("brand_new_family", "").0, "other");
        assert_eq!(
            classify("connectable", "suspect_ghost_seed").0,
            "ghost_seed"
        );
        for reason in [
            "客户端被禁止（黑名单），请联系管理组",
            "客户端不在允许列表，请联系管理组",
        ] {
            assert_eq!(classify("Mozilla/5.0", reason).0, "client_blocked");
        }
    }

    #[test]
    fn aggregate_merges_hits_and_sorts_by_hits() {
        let rows = vec![
            ev("ghost:aaaa", 2, 1),
            ev("ghost:bbbb", 3, 2),
            ev("speed:9", 1, 3),
        ];
        let out = aggregate(&rows);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].code, "ghost_seed");
        assert_eq!(out[0].hits, 5);
        assert_eq!(out[0].first_seen, Some(rows[0].first_seen));
        assert_eq!(out[0].last_seen, Some(rows[1].last_seen));
        assert_eq!(out[1].code, "speed_over_ceiling");
        assert_eq!(out[1].torrent_ids, vec![9]);
    }

    #[test]
    fn aggregate_dedupes_torrent_ids() {
        let rows = vec![ev("speed:9", 1, 1), ev("speed:9", 1, 2)];
        let out = aggregate(&rows);
        assert_eq!(out[0].hits, 2);
        assert_eq!(out[0].torrent_ids, vec![9]);
    }

    #[test]
    fn empty_evidence_yields_no_reasons() {
        assert!(aggregate(&[]).is_empty());
    }
}
