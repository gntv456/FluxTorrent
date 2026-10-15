//! 注册治理四道闸（0370 M4，只做 P0）。
//!
//! NexusPHP 的 USER_LIMIT / 同 IP 限额 / 一次性邮箱，补到 FluxTorrent。
//! 四道闸各自独立、fail-open（读设置失败不阻断注册——治理是加固不是单点）：
//!   ① 用户总量 maxusers：**在注册事务内**校验 + advisory 锁防并发超发
//!      （见 repo/auth.rs；NexusPHP 的 check-then-insert 竞态是本项目改进点）
//!   ② 注册窗口 reg_window：JSON {open,close,slots,label}，窗口外/名额满拒绝
//!   ③ 同 IP 限额 reg_ip_limit：窗口内 reg_ip_signups（sha3 摘要）计数
//!   ④ 一次性邮箱：域名查 disposable_email_domains（含子域后缀匹配）
//!
//! 隐私口径：IP 只以 sha3-256 摘要落库，明文不出本模块；30 天后由
//! worker reg_cleanup 清理（reg_ip_retention_days）。
//! SQL 全部在 repo/register_gates.rs（FRAME §2）；本文件只做闸逻辑与文案。

use crate::errors::{DomainError, DomainResult};
use crate::repo::register_gates as gates_repo;
use crate::repo::Repo;
use crate::state::AppState;

/// IP 摘要（sha3-256 十六进制）。库中永不出现明文 IP。
pub(crate) fn ip_hash(ip: &str) -> String {
    use sha3::Digest;
    let mut h = sha3::Sha3_256::new();
    h.update(ip.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// 读一个 site_settings 文本键（缺行/错行返回 None）。
async fn setting(repo: &Repo, name: &str) -> Option<String> {
    gates_repo::setting(repo, name).await
}

/// 注册窗口闸：空 = 不限制；窗口外或名额满 → 拒绝并给出可行动提示。
async fn check_window(repo: &Repo) -> DomainResult<()> {
    let Some(raw) = setting(repo, "reg_window").await else {
        return Ok(());
    };
    let raw = raw.trim();
    if raw.is_empty() || raw == "null" {
        return Ok(());
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return Ok(());
    };
    let label = v
        .get("label")
        .and_then(|l| l.as_str())
        .unwrap_or("本轮开放")
        .to_string();
    let parse = |k: &str| -> Option<chrono::DateTime<chrono::Utc>> {
        v.get(k)
            .and_then(|x| x.as_str())
            .filter(|s| !s.trim().is_empty())
            .and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s.trim())
                    .ok()
                    .map(|d| d.with_timezone(&chrono::Utc))
            })
    };
    let now = chrono::Utc::now();
    if let Some(open) = parse("open") {
        if now < open {
            return Err(DomainError::Validation(format!(
                "「{label}」尚未开始（{open} 开放注册）"
            )));
        }
    }
    if let Some(close) = parse("close") {
        if now >= close {
            return Err(DomainError::Validation(format!(
                "「{label}」注册已结束"
            )));
        }
    }
    // 名额：窗口内已建账号数（含已封禁——名额是发号量，不是活跃量）
    let slots = v.get("slots").and_then(|x| x.as_i64()).unwrap_or(0);
    if slots > 0 {
        let since = parse("open").unwrap_or(now - chrono::Duration::days(3650));
        let used = gates_repo::users_created_since(repo, since)
            .await
            .unwrap_or(0);
        if used >= slots {
            let close_hint = parse("close")
                .map(|c| format!("，名额每轮重置（本轮至 {c}）"))
                .unwrap_or_default();
            return Err(DomainError::Validation(format!(
                "「{label}」名额已满（{used}/{slots}）{close_hint}"
            )));
        }
    }
    Ok(())
}

/// 同 IP 闸：窗口内该 IP 摘要的注册数达上限即拒。
async fn check_ip_limit(repo: &Repo, ip: &str) -> DomainResult<()> {
    let limit: i64 = setting(repo, "reg_ip_limit")
        .await
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(2);
    if limit <= 0 {
        return Ok(());
    }
    let hours: i64 = setting(repo, "reg_ip_window_hours")
        .await
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(24)
        .clamp(1, 24 * 365);
    let n = gates_repo::ip_signup_count(repo, &ip_hash(ip), hours)
        .await
        .unwrap_or(0);
    if n >= limit {
        return Err(DomainError::Validation(format!(
            "同一网络环境 {hours} 小时内最多注册 {limit} 个账号，请稍后再试"
        )));
    }
    Ok(())
}

/// 一次性邮箱闸：域名（含子域后缀）命中即拒。
/// 可被 reg_block_disposable=no 整体关闭（站长自定策略）。
async fn check_disposable(repo: &Repo, email: &str) -> DomainResult<()> {
    let on = setting(repo, "reg_block_disposable")
        .await
        .map(|v| v.trim().to_ascii_lowercase() != "no")
        .unwrap_or(true);
    if !on {
        return Ok(());
    }
    let Some(domain) = email.rsplit_once('@').map(|(_, d)| d.to_lowercase())
    else {
        return Ok(());
    };
    let hit = gates_repo::disposable_hit(repo, &domain)
        .await
        .unwrap_or(false);
    if hit {
        return Err(DomainError::Validation(
            "本站不接受一次性/临时邮箱，请使用常用邮箱注册".into(),
        ));
    }
    Ok(())
}

/// 三道前置闸（用户总量在事务内另查）。在 register.rs 建号前调用。
pub(crate) async fn check_reg_gates(
    state: &AppState,
    ip: &str,
    email: &str,
) -> DomainResult<()> {
    check_window(&state.repo).await?;
    check_ip_limit(&state.repo, ip).await?;
    check_disposable(&state.repo, email).await?;
    Ok(())
}

/// 建号成功后留痕（摘要）。失败只告警，不阻断注册（附属数据）。
pub(crate) async fn record_ip_signup(repo: &Repo, ip: &str, user_id: i64) {
    if let Err(e) =
        gates_repo::insert_ip_signup(repo, &ip_hash(ip), user_id).await
    {
        tracing::warn!(%e, user = user_id, "reg_ip_signups 留痕失败");
    }
}
