//! 全站维护模式（0462，对标 NexusPHP `main.site_online` / Unit3D `artisan down`）。
//!
//! 口径（两个标杆共同验证过的 PT 特有设计）：
//! - **tracker / RSS / torznab 不拦**——维护期间做种与下载不中断。把全站
//!   peer 的做种打断，恢复后 H&R、魔力结算全是事故，所以 announce 域
//!   （独立进程）与本 API 内的 RSS 口都不进这道闸门；
//! - **staff（class_id >= 90）穿透**：维护中还能进后台干活/随时关闭，
//!   判定与 `require_staff` / 前端 `isStaffClass` 同一等级线；
//! - **放行白名单**：登录/登出（staff 也要能登进来）、探针、安装向导、
//!   site-profile（前端维护页要渲染品牌与维护文案）。
//!
//! 开关读取走 `maintenance::flag()`（30s TTL 缓存 + 保存口主动失效，
//! 与 `modules::ModuleFlags` 同一套路）。**fail-open**：DB 读不到按
//! 「未维护」放行——开关读挂把全站打成 503 比维护模式晚生效 30s 更糟。

use std::sync::Arc;
use std::time::{Duration, Instant};

use actix_web::web;
use tokio::sync::RwLock;

use crate::state::AppState;

/// 白名单分两档（2026-11-06 复核修正）：
/// - EXACT：全等才放行。`/api/v1/me` 若按前缀放行会盖住 `/me/vouchers`
///   等一串业务口（复核实测漏拦），所以自助短路径一律进这档；
/// - SEGMENT：全等或**路径段子路径**（`p/`）放行。只有确有子路径的口
///   进这档（/auth/login 无子路径，但语义上按段挂最稳）。
const EXACT: &[&str] = &[
    "/api/v1/me",
    "/api/v1/setup",
    "/api/v1/about",
    "/api/v1/compat/meta",
    "/api/v1/compat/nexusphp/getrss.php",
    "/api/v1/health",
    "/api/v1/ready",
    "/api/v1/site-profile",
];
const SEGMENT: &[&str] = &[
    "/api/v1/auth/login",
    "/api/v1/auth/logout",
    "/api/v1/auth/email-policy",
    // 强制改密自救（同 auth_infra 的闸门口径）：staff 拿临时密码登进来
    // 后被这里拦死就永远关不掉维护了
    "/api/v1/me/password",
];

/// 白名单判定：EXACT 全等；SEGMENT 全等或后跟 `/`。
/// 裸 starts_with 被 2026-11-06 复核证伪——`me` 是 `medal-wall`、
/// `messages` 的前缀，会把一整族业务口放进维护期。
fn path_allowed(path: &str) -> bool {
    EXACT.iter().any(|p| path == *p)
        || SEGMENT
            .iter()
            .any(|p| path == *p || path.starts_with(&format!("{}/", p)))
}

/// 维护开关缓存（TTL 30s + 保存口主动失效）。单值开关不需要 ModuleFlags
/// 的整表加载，读一条键值即可。
static FLAG: RwLock<Option<(bool, Instant)>> = RwLock::const_new(None);

/// 当前是否处于维护模式（含缓存；DB 读失败按 false，见模块注释）
pub async fn on(db: &sqlx::PgPool) -> bool {
    {
        let guard = FLAG.read().await;
        if let Some((v, at)) = guard.as_ref() {
            if at.elapsed() < Duration::from_secs(30) {
                return *v;
            }
        }
    }
    let fresh = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'maintenance_mode'",
    )
    .fetch_optional(db)
    .await
    .map(|v| v.as_deref().map(str::trim) == Some("yes"))
    .unwrap_or(false);
    *FLAG.write().await = Some((fresh, Instant::now()));
    fresh
}

/// 缓存失效（settings 保存口调用；RSS/compat 不读这个缓存无影响）
pub async fn invalidate() {
    *FLAG.write().await = None;
}

/// 维护闸门中间件：维护模式开启时，非 staff 的 /api/v1 业务请求
/// 一律 4104/503；白名单与 tracker/RSS 除外（见模块注释）。
/// 挂载顺序在 setup_gate 之后、module_gate 之前（先「站开着吗」再「功能开吗」）。
pub async fn maintenance_mw(
    req: actix_web::dev::ServiceRequest,
    next: actix_web::middleware::Next<impl actix_web::body::MessageBody>,
) -> actix_web::Result<
    actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
> {
    let path = req.path();
    // tracker 是独立进程不经这里；RSS/torznab/getrss 别名刻意不拦（模块注释）
    if !path.starts_with("/api/v1")
        || path.starts_with("/api/v1/rss")
        || path.starts_with("/api/v1/torznab")
        || path.starts_with("/api/v1/newznab")
        // 管理端点免检（§5.5 与 module_gate 同则）：站长必须始终能进设置
        // 中心去「关闭维护」——真机验证抓到的自锁：开了维护后保存口
        // 自己被拦死，唯一的出路是直连改库。免检的口子自身有
        // require_perm(SETTINGS_VIEW) 等级门，不是匿名通道。
        || path.starts_with("/api/v1/admin")
        || path_allowed(path)
    {
        return next.call(req).await;
    }
    let state = req
        .app_data::<web::Data<Arc<AppState>>>()
        .cloned()
        .expect("AppState registered");
    if !on(&state.repo.db).await {
        return next.call(req).await;
    }
    // staff 穿透：复用 require_auth 的完整校验（撤销/封禁/降级即时生效）。
    // 匿名与非 staff 同样被拦——维护页对未登录访客也要能渲染（site-profile
    // 已在白名单）。token 无效这里不细分 401/403，统一落维护闸（更诚实：
    // 维护期间没必要暴露「这个 token 曾经是谁」）。
    let staff = crate::http::optional_auth(req.request(), &state)
        .await
        .map(|u| u.class_id >= 90)
        .unwrap_or(false);
    if staff {
        return next.call(req).await;
    }
    Err(actix_web::Error::from(
        crate::errors::DomainError::Maintenance,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 白名单自检：核心自救口都在；路径段语义不放过长路径邻居。
    /// 2026-11-06 复核抓到的回归形态：`starts_with("/api/v1/me")` 放行
    /// 了 /me/vouchers、/messages/inbox、/medal-wall（维护关时都是活口）。
    #[test]
    fn allowlist_contract() {
        // 自救口与维护页依赖：全等与子路径都要放行
        for p in [
            "/api/v1/auth/login",
            "/api/v1/me",
            "/api/v1/me/password",
            "/api/v1/site-profile",
            "/api/v1/health",
            "/api/v1/auth/email-policy",
            "/api/v1/compat/nexusphp/getrss.php",
        ] {
            assert!(path_allowed(p), "missing {p}");
        }
        // 长路径邻居不得被短前缀误放行（上一版就是这里漏的）
        for p in [
            "/api/v1/messages/inbox",
            "/api/v1/me/vouchers",
            "/api/v1/medal-wall",
            "/api/v1/setup-evil",
        ] {
            assert!(!path_allowed(p), "over-match {p}");
        }
        // RSS 口必须走「前缀排除」而不是混进白名单再排除——两者语义不同，
        // 这里锁的是「没人把 RSS 前缀误登进白名单」
        assert!(EXACT
            .iter()
            .chain(SEGMENT)
            .all(|a| { !a.starts_with("/api/v1/rss") }));
    }
}
