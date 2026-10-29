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
    // 名额（P-684 口径）：窗口内**直接注册**数（invited_by IS NULL）——
    // 邀请码注册消耗的是邀请人配额，不占开放轮名额（站长说「开放 100 人」
    // 就是 100 个免邀请码入口）。含已封禁账号——名额是发号量不是活跃量。
    let slots = v.get("slots").and_then(|x| x.as_i64()).unwrap_or(0);
    if slots > 0 {
        let since = parse("open").unwrap_or(now - chrono::Duration::days(3650));
        let used = gates_repo::direct_signups_since(repo, since)
            .await
            .unwrap_or(0);
        if used >= slots {
            let close_hint = parse("close")
                .map(|c| format!("，本轮至 {c}"))
                .unwrap_or_default();
            return Err(DomainError::Validation(format!(
                "「{label}」名额已满（{slots} 个免邀请码名额已全部用完）{close_hint}，可通过邀请码注册"
            )));
        }
    }
    Ok(())
}

/// 同 IP 闸：窗口内该 IP 摘要的注册数达上限即拒。
async fn check_ip_limit(repo: &Repo, ip: &str) -> DomainResult<()> {
    // 缺省 5（P-687，原 2）：家庭/办公宽带 NAT 出口后面是整个家庭乃至
    // 楼宇——开放注册轮里「同 IP=2」会把第三个真实访客拦在门外
    // （现网实测：两位会员注册后，同小区第三位被拦）。0=不限，后台
    // 「同 IP 注册上限」可按站型收紧。
    let limit: i64 = setting(repo, "reg_ip_limit")
        .await
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(5);
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
            "同一网络环境（如家庭/公司宽带）{hours} 小时内最多注册 {limit} 个账号。\
             如确认非重复注册，可换用邀请码注册，或联系管理组协助"
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
///
/// P-689：`using_invite` 为真时跳过「注册窗口」「同 IP 限额」——这两道
/// 闸约束的是免邀请码的开放入口，而拦截文案本身写着「可通过邀请码
/// 注册」「可换用邀请码注册」；把邀请注册也拦掉会让指引变成死路。
/// 一次性邮箱闸对两种入口都生效（站点邮箱政策与入站凭据无关）。
pub(crate) async fn check_reg_gates(
    state: &AppState,
    ip: &str,
    email: &str,
    using_invite: bool,
) -> DomainResult<()> {
    if !using_invite {
        check_window(&state.repo).await?;
        check_ip_limit(&state.repo, ip).await?;
    }
    check_disposable(&state.repo, email).await?;
    Ok(())
}

/// 直接注册的「名额 / 同 IP 额度」临界区锁（P-689）。
///
/// 这两道闸是 check-then-insert：窗口名额与 IP 计数都在建号事务之外
/// 先读后写，4 个并发请求会各自读到「还剩 1 个」并一起建号（实测
/// 4/4 超额）。这里用 PG 会话级 advisory lock 把「检查 → 建号」
/// 串成一段；锁连接**独立于连接池**（`connect_options` 新建），
/// 因为持锁期间 `register_user` 还要从池里取连接——若锁也占池，
/// 突发并发会把池抽干（N 个请求各持 1 个再等第 2 个）。
/// 邀请注册不占名额/IP 额度，不取这把锁。
pub(crate) struct DirectGateLock {
    conn: sqlx::PgConnection,
}

/// 与 register_user 的 maxusers 锁（`0x7265_675f_6361_7001`）不同键：
/// 两把锁的获取顺序固定为 GATE → CAP，不存在交叉等待。
const DIRECT_GATE_LOCK: i64 = 0x7265_675f_6469_7201;

impl DirectGateLock {
    /// 取锁失败按 fail-open 降级（治理是加固，不能把注册整体打停）。
    pub(crate) async fn acquire(
        db: &sqlx::PgPool,
    ) -> DomainResult<Option<Self>> {
        use sqlx::Connection as _;
        let opts = db.connect_options();
        let mut conn = match sqlx::PgConnection::connect_with(&opts).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "注册闸锁连接失败（降级无锁）");
                return Ok(None);
            }
        };
        if let Err(e) = sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(DIRECT_GATE_LOCK)
            .execute(&mut conn)
            .await
        {
            tracing::warn!(error = %e, "注册闸锁获取失败（降级无锁）");
            return Ok(None);
        }
        Ok(Some(Self { conn }))
    }

    /// 显式解锁；即使解锁语句失败，连接随后 drop 关闭会话也会释放锁。
    pub(crate) async fn release(mut self) {
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(DIRECT_GATE_LOCK)
            .execute(&mut self.conn)
            .await;
    }
}

/// 建号成功后留痕（摘要）。失败只告警，不阻断注册（附属数据）。
pub(crate) async fn record_ip_signup(repo: &Repo, ip: &str, user_id: i64) {
    if let Err(e) =
        gates_repo::insert_ip_signup(repo, &ip_hash(ip), user_id).await
    {
        tracing::warn!(%e, user = user_id, "reg_ip_signups 留痕失败");
    }
}

/// 窗口此刻是否生效（P-686）：reg_window 的 open<=now<close 且未满额。
/// 供注册口的邀请判定与 register-mode 投影共用同一口径；任何解析
/// 失败/未配置都返回 false（fail-closed——预约配置写错时不放行）。
pub(crate) async fn reg_window_active(repo: &Repo) -> bool {
    let Some(raw) = setting(repo, "reg_window").await else {
        return false;
    };
    let raw = raw.trim();
    if raw.is_empty() || raw == "null" {
        return false;
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(raw) else {
        return false;
    };
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
            return false;
        }
    }
    if let Some(close) = parse("close") {
        if now >= close {
            return false;
        }
    }
    let slots = v.get("slots").and_then(|x| x.as_i64()).unwrap_or(0);
    if slots > 0 {
        let since = parse("open").unwrap_or(now - chrono::Duration::days(3650));
        let used = gates_repo::direct_signups_since(repo, since)
            .await
            .unwrap_or(slots);
        if used >= slots {
            return false;
        }
    }
    true
}
