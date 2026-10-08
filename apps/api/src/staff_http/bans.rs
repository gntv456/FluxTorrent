//! 封禁管理（bans）。
//! 从 staff_http.rs 按域拆出。

use actix_web::{delete, get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{bump_guard_ver, require_auth};
use crate::state::AppState;

// ============ staffpanel 管理工具（hxpt faqmanage/modrules/catmanage/bans/massmail 口径） ============

#[get("/admin/bans")]
pub async fn ban_list(
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
pub async fn ban_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BanBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::BANS_MANAGE)
        .await?;
    // 段封禁（审计 10-07 P2）：PT 现实是动态 IP——机房 /24、教育网 /64 复发，
    // 只能按段封才管用。旧实现只收单 IP，而 inet 列本身能存掩码、tracker 侧
    // 也已改成 `ip::text` 取段，这里补上写入口。
    let ip_text = parse_ban_target(&body.ip)?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO ip_bans (ip, reason, banned_by) VALUES ($1::inet, \
         $2, $3) ON CONFLICT (ip) DO UPDATE SET reason = EXCLUDED.reason \
         RETURNING id",
    )
    .bind(&ip_text)
    .bind(&body.reason)
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "ip_ban", None).await;
    bump_guard_ver(&state).await;
    Ok(ok(serde_json::json!({ "id": id, "ip": ip_text })))
}

/// 接受单 IP 或 CIDR 段；规范化输出（去掉多余空格、`/0` 一律拒绝）。
fn parse_ban_target(raw: &str) -> Result<String, DomainError> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(DomainError::Validation("IP 格式无效".into()));
    }
    if let Ok(ip) = s.parse::<std::net::IpAddr>() {
        return Ok(ip.to_string());
    }
    // 解析失败一律回「IP 格式无效」（已译），只有 CIDR 特有的一档才新增句子
    let (base, bits) = s
        .split_once('/')
        .ok_or_else(|| DomainError::Validation("IP 格式无效".into()))?;
    let ip: std::net::IpAddr = base
        .parse()
        .map_err(|_| DomainError::Validation("IP 格式无效".into()))?;
    let bits: u32 = bits
        .parse()
        .map_err(|_| DomainError::Validation("CIDR 前缀长度无效".into()))?;
    let max = if ip.is_ipv4() { 32 } else { 128 };
    if bits == 0 || bits > max {
        return Err(DomainError::Validation("CIDR 前缀长度无效".into()));
    }
    // 掩码归一：10.9.8.77/24 → 10.9.8.0/24，避免同段以不同写法重复登记
    let norm = match ip {
        std::net::IpAddr::V4(v4) => {
            let mask = if bits == 32 {
                !0u32
            } else {
                !0u32 << (32 - bits)
            };
            let net: std::net::Ipv4Addr = (u32::from(v4) & mask).into();
            std::net::IpAddr::V4(net)
        }
        std::net::IpAddr::V6(v6) => {
            let mask = if bits == 128 {
                !0u128
            } else {
                !0u128 << (128 - bits)
            };
            let net: std::net::Ipv6Addr = (u128::from(v6) & mask).into();
            std::net::IpAddr::V6(net)
        }
    };
    Ok(format!("{norm}/{bits}"))
}

#[delete("/admin/bans/{id}")]
pub async fn ban_delete(
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

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct IpBanRow {
    pub(super) id: i32,
    pub(super) ip: String,
    pub(super) reason: Option<String>,
    pub(super) banned_by: Option<String>,
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::parse_ban_target;

    #[test]
    fn single_ip_still_works() {
        assert_eq!(parse_ban_target(" 8.8.8.8 ").unwrap(), "8.8.8.8");
        assert_eq!(parse_ban_target("2001:DB8::1").unwrap(), "2001:db8::1");
    }

    #[test]
    fn cidr_is_normalized_by_prefix() {
        // 同段不同主机写法必须收敛成一条，否则封禁表会长出一堆等价行
        assert_eq!(parse_ban_target("10.9.8.77/24").unwrap(), "10.9.8.0/24");
        assert_eq!(parse_ban_target("10.9.8.0/24").unwrap(), "10.9.8.0/24");
        assert_eq!(parse_ban_target("10.9.8.77/32").unwrap(), "10.9.8.77/32");
        assert_eq!(
            parse_ban_target("2001:db8:abcd:ef::9/48").unwrap(),
            "2001:db8:abcd::/48"
        );
    }

    #[test]
    fn junk_and_whole_internet_are_rejected() {
        for bad in [
            "",
            "  ",
            "not-an-ip",
            "10.9.8.0/",
            "10.9.8.0/33",
            "10.9.8.0/0",
            "0.0.0.0/0",
            "::/0",
            "1.2.3/24",
            "10.9.8.0/abc",
        ] {
            assert!(
                parse_ban_target(bad).is_err(),
                "{bad} 不该被接受为封禁目标"
            );
        }
    }
}
