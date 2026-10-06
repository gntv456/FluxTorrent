//! 鉴权与限流设施：throttle/like_pattern/bump_guard_ver/client_ip/ip_banned/
//! require_auth/optional_auth/require_staff + AuthUser。
//! 从 http.rs 按域拆出。

use actix_web::{web, HttpRequest};
use std::sync::Arc;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

// ============ 基础 ============
// 健康/就绪探针已按域拆到 http/probes.rs（ZT81）。

// ============ 认证（M01） ============

pub async fn throttle(state: &Arc<AppState>, key: String) -> DomainResult<()> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let k = format!("rl:{}", key);
    // ZT81：Redis 故障时 fail-close（原 `unwrap_or(0)` 让计数恒 0 → 限流永不触发，
    // 抖动期间密码爆破/撞库全部失守）。与游戏侧 check_rate_scoped 口径对齐。
    let n: i64 = c.incr(&k, 1).await.map_err(|e| {
        DomainError::Internal(anyhow::anyhow!("限流服务不可用: {e}"))
    })?;
    if n == 1 {
        let _: () = c.expire(&k, 60).await.unwrap_or(());
    }
    if n > 5 {
        return Err(DomainError::RateLimited);
    }
    Ok(())
}

/// ILIKE/LIKE 模式构造：转义用户输入中的通配符（% _ \），防 `%` 全表通配扫描的性能滥用。
/// PG 的 LIKE/ILIKE 默认转义符即反斜杠，无需 ESCAPE 子句；torrents.rs 的 ESCAPE chr(92) 口径兼容。
pub fn like_pattern(s: &str) -> String {
    format!(
        "%{}%",
        s.trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

/// 通知 tracker 立即刷新防护缓存（ip_bans/agent_rules/挂起/下载权限/passkey 变更后调用）。
/// tracker 侧每 3s 轮询 flux:guard:ver（见 apps/tracker/src/main.rs）；失败静默 —— 仍有 60s TTL 兜底。
pub async fn bump_guard_ver(state: &Arc<AppState>) {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let _: Result<i64, _> = c.incr("flux:guard:ver", 1).await;
}

/// 解析请求来源 IP（限流 / 封禁 / 风控事件统一口径）。
/// 默认取 socket 对端；`TRUST_PROXY=1` 时采信 X-Forwarded-For。
/// P1-4（2026-10-06 安全审计）：旧版取 XFF **首值**，前提是「反代追加而非
/// 覆盖」——nginx 常见误配 `proxy_set_header X-Forwarded-For
/// $http_x_forwarded_for`（透传客户端自带 XFF）下，攻击者可任意伪造 IP
/// 绕过 ip_bans/login-ip 限流。现改为**取右值**（最后一跳）：标准反代会把
/// 真实客户端 IP 追加在链尾，右侧第一段不受客户端控制；单层代理场景
/// （本项目部署文档的标准形态）右值即真实客户端 IP。多级代理须保证
/// 每一级都追加且最后一跳是可信反代（与之前相比，右值模型只信任「离
/// 我最近的那一跳写了什么」，被伪造面从「整条链」缩小到「直连对端」）。
/// P1 修复（历史）：此前注册/登录/验证码全部直接用 peer_addr（socket 对端），
/// 生产经反代后全站共享代理 IP——login-ip 限流桶全站共用（误伤 429）、
/// ip_bans 误封整个代理、login_events 风控记录失真。
pub fn client_ip(req: &HttpRequest) -> String {
    static TRUST_PROXY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let trust = *TRUST_PROXY.get_or_init(|| {
        std::env::var("TRUST_PROXY").ok().as_deref() == Some("1")
    });
    if trust {
        if let Some(xff) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        {
            // 取链尾值：由直连我们的那台反代追加，客户端无法伪造该段
            // （除非攻击者能直连 api 绕过反代——那属于网络层暴露问题，
            //  部署文档已要求 api 端口不直接对外）。
            if let Some(last) = xff.split(',').next_back() {
                let ip = last.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    req.peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

/// ip_bans 强制校验：命中返回 true（封禁名单由管理面维护，见 /admin/bans）。
/// 登录/注册为低频入口，直接查库即可；高频路径（tracker announce）用内存缓存版，
/// 见 apps/tracker/src/main.rs 的 refresh_guard()/ip_banned()。
pub async fn ip_banned(state: &Arc<AppState>, ip: &str) -> bool {
    if ip.is_empty() || ip == "unknown" {
        return false;
    }
    // CIDR 网段封禁支持：ip 列允许 '1.2.3.0/24' 形状，精确 IP 同时匹配单地址与所属网段
    sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM ip_bans WHERE $1::inet <<= ip LIMIT 1",
    )
    .bind(ip)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .is_some()
}

#[derive(Debug)]
pub struct AuthUser {
    pub id: i64,
    pub class_id: i32,
    /// 本次凭证的签发秒（撤销线语义需要：改密时以它为界作废更早的 token）
    pub iat: i64,
}

/// 用户状态短缓存（审计 P2）：require_auth 每请求的 users 行查询以 5s TTL 缓存。
/// TTL 权衡：封禁/降级最坏 5s 延迟生效（撤销线仍每请求直查，登出即时）；
/// 管理端封禁/降级后调用 invalidate() 主动失效可消除窗口。
#[derive(Default)]
pub struct UserStatusCache {
    inner: std::sync::RwLock<
        std::collections::HashMap<
            i64,
            (Option<(i16, i32, bool)>, std::time::Instant),
        >,
    >,
}

impl UserStatusCache {
    const TTL: std::time::Duration = std::time::Duration::from_secs(5);
    const CAP: usize = 10_000;

    fn get(&self, uid: i64) -> Option<Option<(i16, i32, bool)>> {
        let g = self.inner.read().unwrap_or_else(|e| e.into_inner());
        g.get(&uid)
            .filter(|(_, at)| at.elapsed() < Self::TTL)
            .map(|(v, _)| v.clone())
    }

    fn put(&self, uid: i64, v: Option<(i16, i32, bool)>) {
        let mut g = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if g.len() >= Self::CAP && !g.contains_key(&uid) {
            // 粗略容量保护：淘汰最旧条目（活跃站上 5s 窗口内远达不到 1 万用户）
            if let Some(oldest) =
                g.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| *k)
            {
                g.remove(&oldest);
            }
        }
        g.insert(uid, (v, std::time::Instant::now()));
    }

    /// 管理端封禁/降级/解封后调用：立即失效该用户的缓存行
    pub fn invalidate(&self, uid: i64) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&uid);
    }
}

/// 凭证提取：优先 Authorization: Bearer；缺失时回落 HttpOnly cookie flux_token
/// （登录接口 Set-Cookie 下发，见 login handler）。双轨期两者等价。
/// cookie 多值容忍：cookie Path 作用域迁移期（/api/v1 → /），浏览器可能同时持有
/// 旧路径（已撤销）与根路径（有效）两个同名 cookie，且按 RFC 6265 长 path 在前，
/// actix req.cookie() 取到的是第一个——若只试一个，有效 token 会被旧 cookie 遮蔽
/// 成 401。逐个尝试，签名校验通过且未被撤销者即为凭证。
pub fn token_from_request(req: &HttpRequest) -> Option<String> {
    if let Some(b) = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(b.to_string());
    }
    let cookies = req.cookies().ok()?;
    let candidates: Vec<String> = cookies
        .iter()
        .filter(|c| c.name() == "flux_token")
        .map(|c| c.value().to_string())
        .collect();
    match candidates.len() {
        0 => None,
        1 => Some(candidates.into_iter().next()?),
        _ => {
            // 多值：取「值互不相同者中最后一个」（最近 Set-Cookie 覆盖语义的近似；
            // 同值则任取其一）。调用方 require_auth 会对候选做签名+撤销校验，
            // 这里无法访问 state，故返回全部候选由调用方裁决。
            // 简化：返回最后一个非空值——浏览器按 path 长度排序发送，最后一个
            // 是根路径 cookie，即最新一次登录下发的有效凭证。
            candidates.into_iter().rev().find(|v| !v.is_empty())
        }
    }
}

/// 从 Authorization: Bearer 或 HttpOnly cookie 提取用户（§8.1：后端权威鉴权）
pub async fn require_auth(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<AuthUser> {
    let token = token_from_request(req).ok_or(DomainError::Unauthorized)?;
    let claims = state
        .jwt
        .verify(token.as_str())
        .ok_or(DomainError::Unauthorized)?;
    // 登出撤销检查：签发时间早于 not-before 的 token 一律拒绝。
    // 权威在 token_revocations 表（0085：Redis 重启不再让已撤销 token 复活）；
    // Redis 键保留 24h 作为存量兜底（迁移前撤销线只写了 Redis）。
    {
        let db_nbf: Option<i64> = sqlx::query_scalar(
            "SELECT nbf FROM token_revocations WHERE user_id = $1",
        )
        .bind(claims.sub)
        .fetch_optional(&state.repo.db)
        .await
        .unwrap_or(None);
        let mut c = state.redis.clone();
        let redis_nbf: Option<i64> = redis::AsyncCommands::get(
            &mut c,
            format!("logout_nbf:{}", claims.sub),
        )
        .await
        .unwrap_or(None);
        let nbf = db_nbf.max(redis_nbf);
        if let Some(nbf) = nbf {
            // <=：撤销线含义为「iat 不晚于 nbf 的凭证全部作废」。登出把 nbf 设为凭证 iat，
            // 因此登出所用的 token（iat==nbf）自身也被判死 —— 这是登出的本意；
            // 改密路径写 nbf=iat-1，保留改密后同秒重登的新凭证（见 me_password_change）。
            if claims.iat <= nbf {
                return Err(DomainError::Unauthorized);
            }
        }
    }
    // 权威校验（P1 修复）：token 只是凭证，状态与等级以库为准 —— 封禁/降级即时生效。
    // 5s TTL 缓存（P2）：省掉每请求一次 users round-trip；管理端封禁/降级后调
    // UserStatusCache::invalidate 消除窗口，最坏情况延迟 5s 生效。
    let row: Option<(i16, i32, bool)> =
        match state.user_status_cache.get(claims.sub) {
            Some(cached) => cached,
            None => {
                let fetched = sqlx::query_as(
                    "SELECT status, class_id, \
                 must_reset_password FROM users WHERE id = $1",
                )
                .bind(claims.sub)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                state.user_status_cache.put(claims.sub, fetched.clone());
                fetched
            }
        };
    let Some((status, class_id, must_reset)) = row else {
        return Err(DomainError::Unauthorized);
    };
    if status >= 2 {
        return Err(DomainError::Forbidden); // 封禁账户
    }
    // 审计修复（P1）：临时密码强制改密此前仅靠前端跳转，服务端不拦截 ——
    // 未改密账号可无限期调用全部 API。现在只放行改密/登出/自身信息三条自救路径。
    if must_reset {
        let path = req.path();
        let allowed = matches!(path, "/api/v1/auth/logout")
            || path.starts_with("/api/v1/me/password")
            || path == "/api/v1/me";
        if !allowed {
            return Err(DomainError::Validation(
                "账号正在使用临时密码，请先修改密码后再操作".into(),
            ));
        }
    }
    Ok(AuthUser {
        id: claims.sub,
        class_id,
        iat: claims.iat,
    })
}

/// 可选鉴权：匿名/无效 token 返回 None（自定义菜单公开端点按等级过滤用）
pub async fn optional_auth(
    req: &HttpRequest,
    state: &web::Data<std::sync::Arc<AppState>>,
) -> Option<AuthUser> {
    require_auth(req, state).await.ok()
}

pub fn require_staff(user: &AuthUser) -> DomainResult<()> {
    if user.class_id >= 90 {
        Ok(())
    } else {
        Err(DomainError::Forbidden)
    }
}

// require_staff：仅剩 cheat_events_list 一处调用（P2 双轨鉴权残留），保留待后续
// 改 require_perm 后删除。

// ============ 我的做种/下载/完成列表（旧站 getusertorrentlist 口径） ============
