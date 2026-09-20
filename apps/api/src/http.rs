//! HTTP 接口层：路由 + handlers + 鉴权提取器 + 限流。
//! 分层约束（§8.3.1）：本层只做协议适配，业务规则在 domain/repo。

use actix_web::{delete, get, post, put, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use std::sync::Arc;

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::economy_http::spend_spark;
use crate::errors::{DomainError, DomainResult};
use crate::social_http::{
    endangered_list, team_create, team_join, team_leave, team_list, team_mine,
};
use crate::state::AppState;
use crate::torrents;

pub fn v1_scope() -> actix_web::Scope {
    web::scope("/api/v1")
        .service(health)
        .service(crate::auth_http::register)
        .service(crate::auth_http::login)
        .service(crate::auth_http::me)
        .service(crate::auth_http::me_perms)
        .service(crate::auth_http::me_overview)
        .service(crate::auth_http::me_settings_get)
        .service(crate::auth_http::me_settings_put)
        .service(my_torrentlist)
        .service(crate::auth_http::user_torrentlist)
        .service(my_bookmarks)
        .service(crate::auth_http::rotate_passkey)
        .service(crate::auth_http::my_login_history)
        .service(crate::attachment_http::upload_attachment)
        .service(crate::attachment_http::get_attachment)
        .service(crate::auth_http::me_password_change)
        .service(crate::auth_http::user_public_profile)
        .service(crate::torrent_http::list)
        .service(crate::torrent_http::detail)
        .service(crate::torrent_http::torrent_detail_ext)
        .service(crate::torrent_http::torrent_files)
        .service(crate::torrent_http::torrent_thanks)
        .service(crate::torrent_http::comments)
        .service(crate::torrent_http::create_comment)
        .service(crate::torrent_http::delete_comment)
        .service(crate::torrent_http::do_thank)
        .service(crate::torrent_http::do_bookmark)
        .service(crate::torrent_http::edit_torrent)
        .service(crate::torrent_http::set_torrent_price)
        .service(crate::torrent_http::delete_torrent)
        .service(crate::torrent_http::restore_torrent)
        .service(crate::torrent_http::resubmit_torrent)
        .service(crate::publish_http::group_attach)
        .service(crate::publish_http::group_info)
        .service(crate::publish_http::group_subscribe)
        .service(crate::publish_http::group_unsubscribe)
        .service(crate::torrent_http::torrent_snatches)
        .service(crate::torrent_http::torrent_nfo)
        .service(crate::torrent_http::request_reseed)
        .service(crate::torrent_http::torrent_tags)
        .service(crate::torrent_http::torrent_tag_put)
        .service(stats)
        .service(cheat_events_list)
        .service(report_create)
        .service(rss_info)
        .service(news_create)
        .service(news_update)
        .service(news_delete)
        .service(fun_items)
        .service(fun_item_vote)
        .service(fun_item_create)
        .service(fun_item_update)
        .service(fun_item_set_status)
        .service(fun_item_delete)
        .service(link_apply)
        .service(link_admin_list)
        .service(link_update)
        .service(link_delete)
        .service(crate::staff_http::faq_list)
        .service(crate::staff_http::faq_create)
        .service(crate::staff_http::faq_update)
        .service(crate::staff_http::faq_delete)
        .service(crate::staff_http::rules_content)
        .service(crate::staff_http::rule_create)
        .service(crate::staff_http::rule_update)
        .service(crate::staff_http::rule_delete)
        .service(crate::staff_http::category_list)
        .service(crate::staff_http::category_create)
        .service(crate::staff_http::category_update)
        .service(crate::staff_http::category_delete)
        .service(crate::staff_http::ban_list)
        .service(crate::staff_http::ban_create)
        .service(crate::staff_http::ban_delete)
        .service(crate::staff_http::freeleech_set)
        .service(crate::staff_http::freeleech_clear)
        .service(crate::staff_http::freeleech_update)
        .service(crate::staff_http::freeleech_delete)
        .service(crate::staff_http::freeleech_list)
        .service(crate::staff_http::staffmess_send)
        .service(crate::staff_http::admin_add_user)
        .service(crate::staff_http::admin_amount_bonus)
        .service(crate::staff_http::warned_list)
        .service(crate::staff_http::warn_user)
        .service(crate::staff_http::unwarn_user)
        .service(crate::staff_http::ipcheck)
        .service(crate::staff_http::maxlogin)
        .service(crate::staff_http::admin_amount_upload)
        .service(crate::staff_http::admin_reset_pass)
        .service(crate::staff_http::admin_delete_disabled)
        .service(crate::staff_http::emailban_list)
        .service(crate::staff_http::emailban_create)
        .service(crate::staff_http::emailban_delete)
        .service(crate::staff_http::test_ip)
        .service(crate::staff_http::admin_stats)
        .service(crate::staff_http::clear_cache)
        .service(crate::staff_http::seed_stats)
        .service(crate::staff_http::do_cleanup)
        .service(crate::staff_http::ad_list)
        .service(crate::staff_http::ad_create)
        .service(crate::staff_http::ad_update)
        .service(crate::staff_http::ad_toggle)
        .service(crate::staff_http::ad_delete)
        .service(crate::staff_http::not_connectable)
        .service(crate::staff_http::uploaders)
        .service(crate::staff_http::all_agents)
        .service(crate::staff_http::poll_overview)
        .service(crate::staff_http::db_stats)
        .service(crate::staff_http::sys_log)
        .service(crate::staff_http::locations)
        .service(crate::staff_http::donate_state)
        .service(crate::staff_http::donate_topup)
        .service(crate::staff_http::donate_order_status)
        .service(crate::staff_http::donate_notify)
        .service(crate::staff_http::donate_order)
        .service(crate::staff_http::site_profile)
        .service(crate::staff_http::site_type_pack_list)
        .service(crate::staff_http::site_type_pack_apply)
        .service(crate::staff_http::site_type_pack_diff)
        .service(crate::staff_http::site_type_pack_save)
        .service(crate::staff_http::massmail_list)
        .service(crate::staff_http::massmail_send)
        .service(medal_wall)
        .service(contest_list)
        .service(contest_join)
        .service(frame_list)
        .service(frame_equip)
        .service(gomoku_create)
        .service(gomoku_join)
        .service(gomoku_move)
        .service(gomoku_get)
        .service(home_sections)
        .service(admin_home_layout_put)
        .service(crate::publish_http::upload)
        .service(crate::publish_http::ptgen)
        .service(crate::publish_http::download)
        .service(crate::invite_http::issue_invite_handler)
        .service(crate::invite_http::redeem_invite_handler)
        .service(crate::invite_http::list_invites_handler)
        .service(crate::invite_http::invites_status_handler)
        .service(crate::invite_http::email_invite_handler)
        .service(endangered_list)
        .service(team_create)
        .service(team_join)
        .service(team_leave)
        .service(team_list)
        .service(team_mine)
        .service(crate::auth_http::logout)
}

// ============ 基础 ============

#[get("/health")]
async fn health() -> impl Responder {
    ok(serde_json::json!({ "status": "up", "service": "flux-api" }))
}

// ============ 认证（M01） ============

pub async fn throttle(state: &Arc<AppState>, key: String) -> DomainResult<()> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let k = format!("rl:{}", key);
    let n: i64 = c.incr(&k, 1).await.unwrap_or(0);
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
/// 默认取 socket 对端；`TRUST_PROXY=1` 时改信 X-Forwarded-For 首值——仅当 api 只能经
/// 可信反代访问时启用（直连暴露时客户端可伪造 XFF 绕过限流/封禁）。
/// P1 修复：此前注册/登录/验证码全部直接用 peer_addr（socket 对端），生产经反代后
/// 全站共享代理 IP——login-ip 限流桶全站共用（误伤 429）、ip_bans 误封整个代理、
/// login_events 风控记录失真。
pub fn client_ip(req: &HttpRequest) -> String {
    static TRUST_PROXY: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    let trust =
        *TRUST_PROXY.get_or_init(|| std::env::var("TRUST_PROXY").ok().as_deref() == Some("1"));
    if trust {
        if let Some(xff) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
        {
            // 取链路首值（最接近真实客户端的一段）；反代应追加而非覆盖
            if let Some(first) = xff.split(',').next() {
                let ip = first.trim();
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
    sqlx::query_scalar::<_, i32>("SELECT 1 FROM ip_bans WHERE $1::inet <<= ip LIMIT 1")
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
        std::collections::HashMap<i64, (Option<(i16, i32, bool)>, std::time::Instant)>,
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
            if let Some(oldest) = g.iter().min_by_key(|(_, (_, at))| *at).map(|(k, _)| *k) {
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
fn token_from_request(req: &HttpRequest) -> Option<String> {
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
        let db_nbf: Option<i64> =
            sqlx::query_scalar("SELECT nbf FROM token_revocations WHERE user_id = $1")
                .bind(claims.sub)
                .fetch_optional(&state.repo.db)
                .await
                .unwrap_or(None);
        let mut c = state.redis.clone();
        let redis_nbf: Option<i64> =
            redis::AsyncCommands::get(&mut c, format!("logout_nbf:{}", claims.sub))
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
    let row: Option<(i16, i32, bool)> = match state.user_status_cache.get(claims.sub) {
        Some(cached) => cached,
        None => {
            let fetched = sqlx::query_as(
                "SELECT status, class_id, must_reset_password FROM users WHERE id = $1",
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

fn require_staff(user: &AuthUser) -> DomainResult<()> {
    if user.class_id >= 90 {
        Ok(())
    } else {
        Err(DomainError::Forbidden)
    }
}

// require_staff：仅剩 cheat_events_list 一处调用（P2 双轨鉴权残留），保留待后续
// 改 require_perm 后删除。

// ============ 我的做种/下载/完成列表（旧站 getusertorrentlist 口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SnatchRow {
    torrent_id: i64,
    name: String,
    size: i64,
    seeders: i32,
    leechers: i32,
    seeding: bool,
    leeching: bool,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(rename = "done")]
    uploaded_here: i64,
}

#[derive(Deserialize)]
struct SnatchQuery {
    /// seeding / leeching / completed / uploads
    kind: Option<String>,
    limit: Option<i64>,
}

#[get("/me/torrentlist")]
async fn my_torrentlist(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SnatchQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<SnatchRow> = match q.kind.as_deref() {
        Some("uploads") => sqlx::query_as(
            "SELECT t.id AS torrent_id, t.name, t.size, t.seeders, t.leechers, \
             false AS seeding, false AS leeching, NULL::timestamptz AS completed_at, 0::bigint AS uploaded_here \
             FROM torrents t WHERE t.owner_id = $1 AND t.approval_status = 1 \
             ORDER BY t.id DESC LIMIT $2",
        ),
        Some("completed") => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.completed_at IS NOT NULL \
             ORDER BY s.completed_at DESC LIMIT $2",
        ),
        // 默认做种中
        _ => sqlx::query_as(
            "SELECT s.torrent_id, t.name, t.size, t.seeders, t.leechers, s.seeding, s.leeching, \
             s.completed_at, s.uploaded AS uploaded_here \
             FROM snatches s JOIN torrents t ON t.id = s.torrent_id \
             WHERE s.user_id = $1 AND s.seeding \
             ORDER BY s.torrent_id DESC LIMIT $2",
        ),
    }
    .bind(auth.id)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// 我的收藏列表（usercp 收藏夹口径）：bookmark 时间倒序
#[derive(Deserialize)]
struct BookmarksQuery {
    limit: Option<i64>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
struct BookmarkRow {
    torrent_id: i64,
    name: String,
    small_descr: Option<String>,
    size: i64,
    seeders: i32,
    leechers: i32,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[get("/me/bookmarks")]
async fn my_bookmarks(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<BookmarksQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<BookmarkRow> = sqlx::query_as(
        "SELECT t.id AS torrent_id, t.name, t.small_descr, t.size, t.seeders, t.leechers, \
         b.created_at FROM bookmarks b JOIN torrents t ON t.id = b.torrent_id \
         WHERE b.user_id = $1 AND t.approval_status = 1 \
         ORDER BY b.created_at DESC, t.id DESC LIMIT $2",
    )
    .bind(auth.id)
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 统计（M10 / tracker 对接预演） ============

#[get("/stats")]
async fn stats(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    require_auth(&req, &state).await?;
    // 对象级读缓存（0069）：/stats 是全站聚合查询，首页/移动壳高频拉取。
    // TTL 60s 兜底（worker 每 60s 刷快照，口径一致）；Redis 故障直查库（fail-open 不阻断）。
    let key = "cache:stats:v1";
    let mut c = state.redis.clone();
    let hit: Option<String> = redis::AsyncCommands::get(&mut c, key).await.unwrap_or(None);
    if let Some(json) = hit {
        return Ok(ok(
            serde_json::from_str::<serde_json::Value>(&json).unwrap_or(serde_json::Value::Null)
        ));
    }
    let val = serde_json::to_value(torrents::site_stats(&state.repo.db).await?)
        .unwrap_or(serde_json::Value::Null);
    let _: Result<(), _> = redis::AsyncCommands::set_ex(&mut c, key, val.to_string(), 60u64).await;
    Ok(ok(val))
}

// ============ 作弊探测（0069 cheat_events 闭环查询端：tracker 拒绝 → worker 落库 → 后台可查） ============

#[derive(Deserialize)]
struct CheatEventsQuery {
    limit: Option<i64>,
}

/// agent_rules 黑白名单命中记录（staff 专用）：按最近命中倒序
#[get("/admin/cheat-events")]
async fn cheat_events_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<CheatEventsQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    require_staff(&auth)?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let rows: Vec<(
        i64,
        i64,
        String,
        Option<String>,
        String,
        i64,
        chrono::DateTime<chrono::Utc>,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, user_id, agent, peer_ip, reason, hits, first_seen, last_seen \
             FROM cheat_events ORDER BY last_seen DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = rows
        .into_iter()
        .map(|(id, uid, agent, ip, reason, hits, first, last)| {
            serde_json::json!({
                "id": id, "user_id": uid, "agent": agent, "peer_ip": ip,
                "reason": reason, "hits": hits,
                "first_seen": first, "last_seen": last,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({ "items": items })))
}

// ============ 举报信箱（用户提交举报，进管理后台审核队列） ============

#[derive(Deserialize)]
struct ReportReq {
    ref_type: String,
    ref_id: i64,
    reason: String,
}

#[post("/reports")]
async fn report_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ReportReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let allowed = ["torrent", "comment", "user", "subtitle", "forum"];
    if !allowed.contains(&body.ref_type.as_str()) {
        return Err(DomainError::Validation("非法的举报对象".into()));
    }
    if body.reason.trim().is_empty() || body.reason.len() > 500 {
        return Err(DomainError::Validation("举报理由需 1-500 字".into()));
    }
    // 目标存在性校验（此前任意 ref_id 含不存在对象可无限提交）
    let target_table = match body.ref_type.as_str() {
        "torrent" => Some(("torrents", "approval_status = 1")),
        "comment" => Some(("comments", "true")),
        "user" => Some(("users", "status < 2")),
        "subtitle" => Some(("subtitles", "true")),
        "forum" => Some(("topics", "true")),
        _ => None,
    };
    if let Some((table, extra)) = target_table {
        let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id = $1 AND {extra})");
        let exists: bool = sqlx::query_scalar(&sql)
            .bind(body.ref_id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(false);
        if !exists {
            return Err(DomainError::NotFound(body.ref_id));
        }
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO reports (reporter_id, ref_type, ref_id, reason) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(auth.id)
    .bind(&body.ref_type)
    .bind(body.ref_id)
    .bind(body.reason.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[get("/rss-info")]
async fn rss_info(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let passkey: String = sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 绝对地址口径与 rss feed 端点一致：PUBLIC_API_URL 优先，缺省回退请求 Host，
    // 不再硬编 127.0.0.1（生产未配该变量时用户复制的订阅地址必连失败）。
    let base = match std::env::var("PUBLIC_API_URL") {
        Ok(u) if !u.trim().is_empty() => u.trim().trim_end_matches('/').to_string(),
        _ => {
            let host = req
                .headers()
                .get("host")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("127.0.0.1:8080");
            format!("http://{host}")
        }
    };
    Ok(ok(serde_json::json!({
        "urls": [
            { "label": "全部种子", "url": format!("{}/api/v1/rss/{}", base, passkey) },
            { "label": "官种", "url": format!("{}/api/v1/rss/{}?official=true", base, passkey) },
        ],
        "passkey": passkey,
        "base": format!("{}/api/v1/rss/", base),
    })))
}

// ============ 插件：勋章墙 / 大赛 / 头像挂件 / 五子棋 ============

#[derive(serde::Deserialize)]
struct MedalWallQuery {
    /// 用户名/勋章名模糊搜索（ILIKE，2 字符起搜，与论坛搜索同口径）
    #[serde(default)]
    q: Option<String>,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

fn default_page() -> i64 {
    1
}
fn default_per_page() -> i64 {
    12
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct MedalWallUserRow {
    /// 前端跳用户主页用（/users/{id}）：一并带出，省一次按名反查
    user_id: i64,
    username: String,
    medal_name: String,
    asset_ref: Option<String>,
    rarity: Option<String>,
    wearing: bool,
    granted_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize)]
struct MedalWallUser {
    user_id: i64,
    username: String,
    medal_count: i64,
    medals: Vec<MedalWallEntryRich>,
}

#[derive(serde::Serialize)]
struct MedalWallEntryRich {
    medal_name: String,
    asset_ref: Option<String>,
    rarity: Option<String>,
    wearing: bool,
    granted_at: chrono::DateTime<chrono::Utc>,
}

/// 勋章墙（medal_wall.php 口径）：全部用户「持有」的勋章展示墙。
/// 口径=持有且未过期（页面副题「全站用户获得的勋章展示」）；wearing 是单佩戴位
/// （medal_wear 佩戴前会摘掉其余），不能作为墙体过滤——否则每人最多显示一枚。
/// 返回按用户聚合（一卡一人，墙的形态）；q 搜用户名/勋章名（2 字符起）；分页信封与
/// admin 列表同构（rows/total/page/per_page）。行查询 + Rust 线性分组——SQL 侧 jsonb_agg
/// 会让每行都背一遍用户名，且「用户名命中→整人进墙」的搜索语义用 OR 谓词更直白。
#[get("/medal-wall")]
async fn medal_wall(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<MedalWallQuery>,
) -> DomainResult<impl Responder> {
    // 搜索语义：q 命中用户名 → 该用户全部勋章都进墙；只命中勋章名 → 只进那几枚。
    // 一条 OR 谓词即两义齐备，无需先查用户再回表。
    // 三段 WHERE 只从「有无关键字」二选一，值不拼接（关键字走 $1 绑定）——format! 仅作模板拼装。
    let has_kw =
        q.q.as_deref()
            .map(str::trim)
            .filter(|s| s.len() >= 2)
            .is_some();
    let pattern = if has_kw {
        Some(crate::http::like_pattern(
            q.q.as_deref().unwrap_or("").trim(),
        ))
    } else {
        None
    };
    let sql_where = if has_kw {
        "WHERE (um.expires_at IS NULL OR um.expires_at > now()) \
         AND (u.username ILIKE $1 OR m.name ILIKE $1)"
    } else {
        "WHERE um.expires_at IS NULL OR um.expires_at > now()"
    };
    // 计数子查询与行查询必须同口径（total 与 rank 窗口来自同一份谓词）——别名带 2 后缀
    let count_where = if has_kw {
        "WHERE (um2.expires_at IS NULL OR um2.expires_at > now()) \
         AND (u2.username ILIKE $1 OR m2.name ILIKE $1)"
    } else {
        "WHERE um2.expires_at IS NULL OR um2.expires_at > now()"
    };
    let (offset, limit) = crate::dto::page_window(q.page, q.per_page);

    // 按「持勋数降序、用户名」的全序给用户编 rank，再按 rank 区间取页窗口——
    // LIMIT 生效在用户维度而非 (user, medal) 行维度（否则一人 10 枚会吃掉整页，
    // 回到「只见一人」的老问题）。
    let rows: Vec<MedalWallUserRow> = sqlx::query_as(&format!(
        "SELECT user_id, username, medal_name, asset_ref, rarity, wearing, granted_at FROM ( \
            SELECT u.id AS user_id, u.username, m.name AS medal_name, m.asset_ref, m.rarity, um.wearing, um.granted_at, m.id AS medal_id, \
                   dense_rank() OVER (ORDER BY agg.cnt DESC, u.username) AS user_rank \
            FROM ( \
                SELECT u2.id AS uid, count(*) AS cnt FROM user_medals um2 \
                JOIN users u2 ON u2.id = um2.user_id JOIN medals m2 ON m2.id = um2.medal_id \
                {count_where} GROUP BY u2.id \
            ) agg JOIN users u ON u.id = agg.uid \
            JOIN user_medals um ON um.user_id = u.id JOIN medals m ON m.id = um.medal_id \
            {row_where} \
         ) ranked WHERE user_rank >= $2 AND user_rank < $2 + $3 ORDER BY user_rank, medal_id",
        row_where = sql_where,
        count_where = count_where,
    ))
    .bind(&pattern)
    .bind(offset + 1) // rank 从 1 起：第 page 页 = rank ∈ [offset+1, offset+limit]
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // total = 口径下的用户数（与 rank 窗口同一份 count_where）
    let total_users: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM ( \
            SELECT u2.id FROM user_medals um2 \
            JOIN users u2 ON u2.id = um2.user_id JOIN medals m2 ON m2.id = um2.medal_id \
            {count_where} GROUP BY u2.id \
         ) t",
        count_where = count_where,
    ))
    .bind(&pattern)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // Rust 侧聚合：SQL 已按 user_rank 排序，同用户行连续，一次线性扫描分组
    let mut users: Vec<MedalWallUser> = Vec::new();
    for r in rows {
        let entry = MedalWallEntryRich {
            medal_name: r.medal_name,
            asset_ref: r.asset_ref,
            rarity: r.rarity,
            wearing: r.wearing,
            granted_at: r.granted_at,
        };
        match users.last_mut() {
            Some(u) if u.username == r.username => {
                u.medal_count += 1;
                u.medals.push(entry);
            }
            _ => users.push(MedalWallUser {
                user_id: r.user_id,
                username: r.username,
                medal_count: 1,
                medals: vec![entry],
            }),
        }
    }

    Ok(ok(serde_json::json!({
        "rows": users,
        "total": total_users,
        "page": q.page.max(1),
        "per_page": q.per_page,
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct ContestInfo {
    id: i32,
    title: String,
    descr: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    is_active: bool,
    entries: i64,
    #[sqlx(default)]
    leader: Option<String>,
    #[sqlx(default)]
    leader_score: Option<i32>,
}

#[get("/contests")]
async fn contest_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let mut rows: Vec<ContestInfo> = sqlx::query_as(
        "SELECT c.id, c.title, c.descr, c.starts_at, c.ends_at, c.is_active, \
            (SELECT count(*) FROM contest_entries e WHERE e.contest_id = c.id) AS entries, \
            (SELECT u.username FROM contest_entries e JOIN users u ON u.id = e.user_id \
             WHERE e.contest_id = c.id ORDER BY e.score DESC LIMIT 1) AS leader, \
            (SELECT e.score FROM contest_entries e WHERE e.contest_id = c.id ORDER BY e.score DESC LIMIT 1) AS leader_score \
         FROM contests c ORDER BY c.is_active DESC, c.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = &mut rows;
    Ok(ok(rows))
}

#[post("/contests/{id}/join")]
async fn contest_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let inserted = sqlx::query(
        "INSERT INTO contest_entries (contest_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(*path)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("已报名".into()));
    }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct FrameRow {
    id: i32,
    name: String,
    css: String,
    #[sqlx(default)]
    image_url: Option<String>,
    price: i32,
}

#[get("/avatar-frames")]
async fn frame_list(state: web::Data<std::sync::Arc<AppState>>) -> DomainResult<impl Responder> {
    let rows: Vec<FrameRow> = sqlx::query_as(
        "SELECT id, name, css, image_url, price FROM avatar_frames ORDER BY sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FrameSetBody {
    frame_id: Option<i32>,
}

/// 佩戴头像挂件（需已购买：简化口径 price=0 免费 / >0 扣魔力）
#[put("/me/avatar-frame")]
async fn frame_equip(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FrameSetBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    match body.frame_id {
        Some(fid) => {
            let price: Option<i32> =
                sqlx::query_scalar("SELECT price FROM avatar_frames WHERE id=$1")
                    .bind(fid)
                    .fetch_optional(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            let Some(price) = price else {
                return Err(DomainError::NotFound(fid as i64));
            };
            if price > 0 {
                let idem = format!("frame:{}:{}", auth.id, fid);
                // 幂等键确定性（frame:{uid}:{fid}）：换戴回已购框架 → Replayed（不重复
                // 扣款）后仍继续佩戴，这正是「重复佩戴不重复收费」的预期语义，显式丢弃。
                let outcome = spend_spark(
                    &state.repo.db,
                    auth.id,
                    price as i64,
                    "shop",
                    &idem,
                    "avatar_frame",
                    fid as i64,
                )
                .await?;
                let _ = outcome;
            }
            sqlx::query("UPDATE users SET avatar_frame_id=$2 WHERE id=$1")
                .bind(auth.id)
                .bind(fid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "equipped": fid })))
        }
        None => {
            sqlx::query("UPDATE users SET avatar_frame_id=NULL WHERE id=$1")
                .bind(auth.id)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "equipped": null })))
        }
    }
}

// ---- 五子棋（wuziqi 口径：建房/加入/落子/棋盘）----

#[derive(serde::Serialize, sqlx::FromRow)]
struct GomokuGame {
    id: i32,
    black_id: i64,
    white_id: Option<i64>,
    board: String,
    turn: String,
    winner_id: Option<i64>,
}

#[post("/gomoku/games")]
async fn gomoku_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO gomoku_games (black_id, board) VALUES ($1, '') RETURNING id",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[post("/gomoku/games/{id}/join")]
async fn gomoku_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let n = sqlx::query(
        "UPDATE gomoku_games SET white_id=$2, updated_at=now() WHERE id=$1 AND white_id IS NULL AND black_id <> $2",
    ).bind(*path).bind(auth.id)
    .execute(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if n.rows_affected() == 0 {
        return Err(DomainError::Validation("对局不存在或已有对手".into()));
    }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(Deserialize)]
struct MoveBody {
    pos: i32,
}

#[post("/gomoku/games/{id}/move")]
async fn gomoku_move(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<MoveBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if !(0..225).contains(&body.pos) {
        return Err(DomainError::Validation("落点越界".into()));
    }
    let g: Option<(i64, Option<i64>, String, String, Option<i64>)> = sqlx::query_as(
        "SELECT black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    )
    .bind(*path)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((black, white, board, turn, winner)) = g else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if winner.is_some() {
        return Err(DomainError::Validation("对局已结束".into()));
    }
    let Some(white) = white else {
        return Err(DomainError::Validation("等待对手加入".into()));
    };
    let my_color = if auth.id == black {
        'b'
    } else if auth.id == white {
        'w'
    } else {
        return Err(DomainError::Forbidden);
    };
    if my_color.to_string() != turn {
        return Err(DomainError::Validation("还没轮到你".into()));
    }
    // 棋盘惰性填充
    let mut cells: Vec<char> = board.chars().collect();
    cells.resize(225, '.');
    if cells[body.pos as usize] != '.' {
        return Err(DomainError::Validation("该点已有棋子".into()));
    }
    cells[body.pos as usize] = my_color;
    let new_board: String = cells.iter().collect();
    let next_turn = if my_color == 'b' { "w" } else { "b" };
    // 胜负判定（四方向五连）
    let won = check_gomoku_win(&cells, body.pos as usize, my_color);
    let winner_id = if won { Some(auth.id) } else { None };
    sqlx::query(
        "UPDATE gomoku_games SET board=$2, turn=$3, winner_id=$4, updated_at=now() WHERE id=$1",
    )
    .bind(*path)
    .bind(&new_board)
    .bind(next_turn)
    .bind(winner_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(
        serde_json::json!({ "board": new_board, "turn": next_turn, "winner": winner_id, "you_won": won }),
    ))
}

#[get("/gomoku/games/{id}")]
async fn gomoku_get(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let g: Option<GomokuGame> = sqlx::query_as(
        "SELECT id, black_id, white_id, board, turn, winner_id FROM gomoku_games WHERE id=$1",
    )
    .bind(*path)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(g) = g else {
        return Err(DomainError::NotFound(*path as i64));
    };
    Ok(ok(g))
}

fn check_gomoku_win(cells: &[char], pos: usize, color: char) -> bool {
    const SIZE: usize = 15;
    let (r, c) = (pos / SIZE, pos % SIZE);
    let dirs: [(isize, isize); 4] = [(0, 1), (1, 0), (1, 1), (1, -1)];
    for (dr, dc) in dirs {
        let mut count = 1;
        for sign in [1, -1] {
            let mut step = 1;
            loop {
                let rr = r as isize + dr * step * sign;
                let cc = c as isize + dc * step * sign;
                if rr < 0 || rr >= SIZE as isize || cc < 0 || cc >= SIZE as isize {
                    break;
                }
                if cells[rr as usize * SIZE + cc as usize] != color {
                    break;
                }
                count += 1;
                step += 1;
            }
        }
        if count >= 5 {
            return true;
        }
    }
    false
}

// ============ 公告管理（news.php 复刻：发布/编辑/删除，staff 专用） ============

#[derive(Deserialize)]
struct NewsBody {
    title: String,
    body: String,
    #[serde(default)]
    badge: String,
}

#[post("/admin/news")]
async fn news_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    if body.title.trim().is_empty() || body.body.trim().is_empty() {
        return Err(DomainError::Validation("标题和正文不能为空".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO announcements (title, body, badge, author_id, sort)          VALUES ($1, $2, $3, $4, (SELECT COALESCE(max(sort),0)+10 FROM announcements)) RETURNING id",
    )
    .bind(body.title.trim())
    .bind(&body.body)
    .bind(if body.badge.trim().is_empty() { "公告" } else { body.badge.trim() })
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "news_create", None).await;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/admin/news/{id}")]
async fn news_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<NewsBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    let updated =
        sqlx::query("UPDATE announcements SET title = $2, body = $3, badge = $4 WHERE id = $1")
            .bind(*path)
            .bind(body.title.trim())
            .bind(&body.body)
            .bind(if body.badge.trim().is_empty() {
                "公告"
            } else {
                body.badge.trim()
            })
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/news/{id}")]
async fn news_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_any_perm(
        &state,
        &auth,
        &[
            crate::authz::perm::NEWS_MANAGE,
            crate::authz::perm::ANNOUNCE_PUBLISH,
        ],
    )
    .await?;
    let deleted = sqlx::query("DELETE FROM announcements WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    if deleted.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "news_delete", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ 趣味盒（fun.php 复刻：浏览/投票/发布/编辑/删除/禁止） ============

#[derive(serde::Serialize, sqlx::FromRow)]
struct FunItemRow {
    id: i32,
    username: Option<String>,
    title: String,
    body: Option<String>,
    status: String,
    added: chrono::DateTime<chrono::Utc>,
    #[sqlx(default)]
    fun_votes: Option<i64>,
    #[sqlx(default)]
    dull_votes: Option<i64>,
    #[sqlx(default)]
    my_vote: Option<String>,
}

/// 趣味盒列表（?status=all 全量需 staff；默认仅 normal）
#[get("/fun/items")]
async fn fun_items(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let staff_view = q.get("status").map(|s| s.as_str()).unwrap_or("") == "all";
    let rows: Vec<FunItemRow> = sqlx::query_as(
        "SELECT f.id, u.username, f.title, f.body, f.status, f.added, \
            (SELECT count(*) FROM fun_item_votes v WHERE v.fun_id = f.id AND v.vote = 'fun') AS fun_votes, \
            (SELECT count(*) FROM fun_item_votes v WHERE v.fun_id = f.id AND v.vote = 'dull') AS dull_votes, \
            (SELECT v.vote FROM fun_item_votes v WHERE v.fun_id = f.id AND v.user_id = $2) AS my_vote \
         FROM fun_items f LEFT JOIN users u ON u.id = f.user_id \
         WHERE ($3::bool OR f.status = 'normal') \
         ORDER BY f.added DESC LIMIT 50",
    )
    .bind(true)
    .bind(auth.id)
    .bind(auth.class_id >= 90 && staff_view)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct FunVoteBody {
    fun_id: i32,
    vote: String,
}

#[post("/fun/items/vote")]
async fn fun_item_vote(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunVoteBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.vote != "fun" && body.vote != "dull" {
        return Err(DomainError::Validation("投票只能是 fun 或 dull".into()));
    }
    let inserted = sqlx::query(
        "INSERT INTO fun_item_votes (fun_id, user_id, vote) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(body.fun_id)
    .bind(auth.id)
    .bind(&body.vote)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("已经投过啦".into()));
    }
    Ok(ok(serde_json::json!({ "voted": body.vote })))
}

#[derive(Deserialize)]
struct FunItemBody {
    title: String,
    #[serde(default)]
    body: Option<String>,
}

/// 发布趣味内容（24h 冷却：最新一条发布不足 24 小时则拒绝，staff 豁免）
#[post("/fun/items")]
async fn fun_item_create(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<FunItemBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("标题不能为空".into()));
    }
    let recent: Option<chrono::DateTime<chrono::Utc>> = sqlx::query_scalar(
        "SELECT max(added) FROM fun_items WHERE status NOT IN ('banned','dull')",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if let Some(t) = recent {
        if chrono::Utc::now() - t < chrono::Duration::hours(24)
            && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE).await
        {
            return Err(DomainError::Validation(
                "最新一条发布不足 24 小时，请稍后再来".into(),
            ));
        }
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO fun_items (user_id, title, body) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(auth.id)
    .bind(body.title.trim())
    .bind(&body.body)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "id": id })))
}

#[put("/fun/items/{id}")]
async fn fun_item_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FunItemBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
        .bind(*path)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if owner != auth.id && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE).await {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("UPDATE fun_items SET title = $2, body = $3 WHERE id = $1")
        .bind(*path)
        .bind(body.title.trim())
        .bind(&body.body)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
struct FunStatusBody {
    status: String,
}

/// 修改状态（含「禁止」banned；staff 或作者对自己可 dull）
#[put("/fun/items/{id}/status")]
async fn fun_item_set_status(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
    body: web::Json<FunStatusBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let allowed = ["normal", "dull", "notfunny", "funny", "veryfunny", "banned"];
    if !allowed.contains(&body.status.as_str()) {
        return Err(DomainError::Validation("非法状态".into()));
    }
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
        .bind(*path)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    let is_staff = auth.class_id >= 90;
    if (body.status == "banned" || owner != auth.id) && !is_staff {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("UPDATE fun_items SET status = $2 WHERE id = $1")
        .bind(*path)
        .bind(&body.status)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "fun_status_change", None)
        .await;
    Ok(ok(serde_json::json!({ "ok": true, "status": body.status })))
}

#[delete("/fun/items/{id}")]
async fn fun_item_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM fun_items WHERE id = $1")
        .bind(*path)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(*path as i64));
    };
    if owner != auth.id && !crate::authz::can(&state, &auth, crate::authz::perm::FUN_MANAGE).await {
        return Err(DomainError::Forbidden);
    }
    sqlx::query("DELETE FROM fun_items WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

// ============ 友情链接管理（linksmanage.php 复刻：申请/审核/编辑/删除） ============

#[derive(Deserialize)]
struct LinkApplyBody {
    name: String,
    url: String,
    #[serde(default)]
    title: Option<String>,
    admin: String,
    email: String,
    reason: String,
}

/// 申请友链（进 pending，staff 审核后 active）
#[post("/links/apply")]
async fn link_apply(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<LinkApplyBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    if body.name.trim().is_empty() || body.url.trim().is_empty() {
        return Err(DomainError::Validation("站点名和 URL 必填".into()));
    }
    if !body.email.contains('@') {
        return Err(DomainError::Validation("邮箱格式无效".into()));
    }
    if body.reason.trim().chars().count() < 20 {
        return Err(DomainError::Validation("申请理由至少 20 字".into()));
    }
    let id: i32 = sqlx::query_scalar(
        "INSERT INTO friend_links (name, url, title, status, applied_by, admin_name, email, reason)          VALUES ($1, $2, $3, 'pending', $4, $5, $6, $7) RETURNING id",
    )
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(&body.title)
    .bind(auth.id)
    .bind(body.admin.trim())
    .bind(body.email.trim())
    .bind(body.reason.trim())
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "link_apply", None).await;
    Ok(ok(serde_json::json!({ "id": id, "status": "pending" })))
}

#[derive(Deserialize)]
struct LinkAdminBody {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    sort: Option<i32>,
    #[serde(default)]
    status: Option<String>,
}

/// 编辑/审核友链（staff；status: pending→active 通过）
#[put("/admin/links/{id}")]
async fn link_update(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<LinkAdminBody>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE).await?;
    if let Some(st) = &body.status {
        if !["pending", "active", "hidden"].contains(&st.as_str()) {
            return Err(DomainError::Validation("非法状态".into()));
        }
    }
    let updated = sqlx::query(
        "UPDATE friend_links SET \
            name = COALESCE($2, name), url = COALESCE($3, url), title = COALESCE($4, title), \
            sort = COALESCE($5, sort), status = COALESCE($6, status) \
         WHERE id = $1",
    )
    .bind(*path)
    .bind(&body.name)
    .bind(&body.url)
    .bind(&body.title)
    .bind(body.sort)
    .bind(&body.status)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(*path));
    }
    state.repo.audit(Some(auth.id), "link_update", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[delete("/admin/links/{id}")]
async fn link_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE).await?;
    sqlx::query("DELETE FROM friend_links WHERE id = $1")
        .bind(*path)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    state.repo.audit(Some(auth.id), "link_delete", None).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct LinkRow {
    id: i32,
    name: String,
    url: String,
    title: Option<String>,
    status: String,
    #[sqlx(default)]
    applied_by: Option<i64>,
    #[sqlx(default)]
    admin_name: Option<String>,
    #[sqlx(default)]
    email: Option<String>,
    #[sqlx(default)]
    reason: Option<String>,
}

/// 友链管理列表（staff，含 pending）
#[get("/admin/links")]
async fn link_admin_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::LINKS_MANAGE).await?;
    let rows: Vec<LinkRow> = sqlx::query_as(
        "SELECT id, name, url, title, status, applied_by, admin_name, email, reason \
         FROM friend_links ORDER BY (status = 'pending') DESC, sort, id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

// ============ 首页（复刻包子站 index.php 五大板块） ============

/// 首页汇总：公告 + 签到日历 + 30 天新增资源统计 + 站点数据 + 娱乐流水
#[get("/home")]
async fn home_sections(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let uid = auth.id;

    // 公告（home-news）：最新一条为头条 + 其余为列表
    let news: Vec<(i32, String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, title, body, badge, created_at FROM announcements ORDER BY id DESC LIMIT 8",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let news_json: Vec<serde_json::Value> = news
        .iter()
        .map(|(id, title, body, badge, ts)| {
            // 公告 body 为富文本 HTML（管理员撰写，NP 口径）：出站前 ammonia 白名单消毒，
            // 剥离 script/事件属性/javascript: 协议 —— 管理员账号被盗也不构成全站存储 XSS
            let safe_body = ammonia::Builder::default().clean(body).to_string();
            serde_json::json!({
                "id": id, "title": title, "body": safe_body, "badge": badge,
                "date": ts.format("%m-%d").to_string(),
            })
        })
        .collect();

    // 签到日历（attendance-card）：当月逐日 + 连签/累计 + 补签卡持有数（0066）
    let makeup_cards: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM shop_orders o JOIN shop_items i ON i.id = o.item_id \
         WHERE o.user_id = $1 AND i.kind IN ('makeup_card','resub_card') \
           AND NOT EXISTS (SELECT 1 FROM resub_uses r WHERE r.idempotency_key = concat('resub:', o.id))",
    )
    .bind(uid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let att: Vec<(chrono::NaiveDate, i32, i64)> = sqlx::query_as(
        "SELECT date, streak, reward FROM attendance WHERE user_id = $1 ORDER BY date",
    )
    .bind(uid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 站点时区 UTC+8（与 /attendance、/checkin 同口径；容器 TZ=UTC 时 Local 会让
    // 首页日历在北京 0-8 点窗口显示「昨日未签」）
    let today = (chrono::Utc::now() + chrono::Duration::hours(8)).date_naive();
    let month_start = chrono::Datelike::with_day(&today, 1).unwrap_or(today);
    let days_in_month = {
        let next_month = if chrono::Datelike::month(&month_start) == 12 {
            chrono::NaiveDate::from_ymd_opt(chrono::Datelike::year(&month_start) + 1, 1, 1)
        } else {
            chrono::NaiveDate::from_ymd_opt(
                chrono::Datelike::year(&month_start),
                chrono::Datelike::month(&month_start) + 1,
                1,
            )
        }
        .unwrap_or(today);
        (next_month.pred_opt().unwrap_or(today) - month_start).num_days() as u32 + 1
    };
    let calendar: Vec<serde_json::Value> = (0..days_in_month)
        .map(|i| {
            let d = month_start + chrono::Duration::days(i as i64);
            let hit = att.iter().find(|(ad, _, _)| *ad == d);
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "day": chrono::Datelike::day(&d),
                "done": hit.is_some(),
                "reward": hit.map(|(_, _, r)| r).unwrap_or(&0),
            })
        })
        .collect();
    let streak = att.iter().rev().next().map(|(_, s, _)| *s).unwrap_or(0);
    let total_days = att.len() as i32;
    let checked_today = att.iter().any(|(d, _, _)| *d == today);

    // 30 天新增资源统计（home-resource-stats）：普通 vs 官种
    let daily: Vec<(chrono::NaiveDate, i64, i64)> = sqlx::query_as(
        "SELECT created_at::date AS d, \
                count(*) FILTER (WHERE NOT official_tag) AS ordinary, \
                count(*) FILTER (WHERE official_tag) AS official \
         FROM torrents WHERE approval_status = 1 AND created_at > now() - interval '30 days' \
         GROUP BY d ORDER BY d",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let by_day: std::collections::HashMap<chrono::NaiveDate, (i64, i64)> =
        daily.iter().map(|(d, o, f)| (*d, (*o, *f))).collect();
    let series: Vec<serde_json::Value> = (0..30)
        .rev()
        .map(|i| {
            let d = today - chrono::Duration::days(i);
            let (o, f) = by_day.get(&d).cloned().unwrap_or((0, 0));
            serde_json::json!({
                "date": d.format("%Y-%m-%d").to_string(),
                "ordinary": o, "official": f, "total": o + f,
            })
        })
        .collect();
    let today_count = by_day.get(&today).map(|(o, f)| o + f).unwrap_or(0);
    let last7: Vec<i64> = (1..=7)
        .map(|i| {
            by_day
                .get(&(today - chrono::Duration::days(i)))
                .map(|(o, f)| o + f)
                .unwrap_or(0)
        })
        .collect();
    let avg7 = if last7.is_empty() {
        0.0
    } else {
        last7.iter().sum::<i64>() as f64 / last7.len() as f64
    };
    let total30: i64 = daily.iter().map(|(_, o, f)| o + f).sum();

    // 站点数据（home-site-data 三列）
    let (users, torrents_n, peers, seeders, leechers, warned, banned, unverified): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT \
            (SELECT count(*) FROM users WHERE status < 2), \
            (SELECT count(*) FROM torrents WHERE approval_status = 1), \
            (SELECT count(*) FROM snatches WHERE seeding OR leeching), \
            (SELECT count(*) FROM snatches WHERE seeding), \
            (SELECT count(*) FROM snatches WHERE leeching), \
            (SELECT count(*) FROM users WHERE status = 1), \
            (SELECT count(*) FROM users WHERE status >= 2), \
            (SELECT count(*) FROM users WHERE must_reset_password)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let (up_sum, down_sum, size_sum): (i64, i64, i64) = sqlx::query_as(
        "SELECT \
            (SELECT COALESCE(sum(uploaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(downloaded),0)::bigint FROM users), \
            (SELECT COALESCE(sum(size),0)::bigint FROM torrents WHERE approval_status = 1)",
    )
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 娱乐流水（幸运大转盘 → 以 spark_ledger 游戏类流水近似）
    let lucky: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT u.username, l.kind, l.amount FROM spark_ledger l \
         JOIN users u ON u.id = l.user_id \
         WHERE l.kind LIKE '%game%' OR l.kind LIKE '%vote%' \
         ORDER BY l.created_at DESC LIMIT 15",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 友情链接
    let links: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT name, url, title FROM friend_links ORDER BY sort")
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;

    Ok(ok(serde_json::json!({
        "news": news_json,
        "attendance": {
            "month": today.format("%Y年%m月").to_string(),
            "streak": streak, "total_days": total_days, "checked_today": checked_today,
            "calendar": calendar, "makeup_cards": makeup_cards,
        },
        "resource_stats": {
            "today": today_count, "avg7": (avg7 * 10.0).round() / 10.0,
            "total30": total30, "series": series,
        },
        "site_data": {
            "users": users, "torrents": torrents_n, "peers": peers,
            "seeders": seeders, "leechers": leechers,
            "warned": warned, "banned": banned, "unverified": unverified,
            "total_upload": up_sum, "total_download": down_sum, "total_size": size_sum,
        },
        "lucky_draw": lucky.iter().map(|(u, k, a)| serde_json::json!({
            "user": u, "kind": k, "amount": a,
        })).collect::<Vec<_>>(),
        "friend_links": links.iter().map(|(n, u, t)| serde_json::json!({
            "name": n, "url": u, "title": t,
        })).collect::<Vec<_>>(),
        // 首页排版（0089）：site_settings.home_layout 原样透传（JSON 数组或空串），
        // 前端空/非法回退默认布局
        "home_layout": crate::http::home_layout_raw(&state.repo.db).await,
    })))
}

/// 读取首页排版配置（0089）：返回原文；库错误/缺行回空串（首页永远可渲染）
pub async fn home_layout_raw(db: &sqlx::PgPool) -> String {
    sqlx::query_scalar::<_, String>("SELECT value FROM site_settings WHERE name = 'home_layout'")
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// 首页板块键白名单（0089）：与前端 HomeSections 渲染分支一一对应
pub const HOME_SECTION_KEYS: &[&str] = &[
    "news",
    "attendance",
    "shoutbox",
    "funbox",
    "resource_stats",
    "site_data",
    "lucky_draw",
    "links",
    "latest",
];

#[derive(Deserialize)]
struct HomeLayoutItem {
    key: String,
    /// 1/2/3 = 1/3、2/3、整行；缺省 0 由前端按板块推荐档处理
    #[serde(default)]
    span: i32,
}

/// 保存首页排版（sysop，0089）：校验 JSON 结构 + 键白名单 + 去重 + span 白名单，
/// 规范化后存 site_settings.home_layout。items 传空数组 = 恢复默认排版。
#[put("/admin/home-layout")]
async fn admin_home_layout_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<Vec<HomeLayoutItem>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::SETTINGS_MANAGE).await?;
    if body.len() > 20 {
        return Err(DomainError::Validation("板块数量至多 20".into()));
    }
    let mut norm: Vec<serde_json::Value> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for it in body.iter() {
        if !HOME_SECTION_KEYS.contains(&it.key.as_str()) {
            return Err(DomainError::Validation(format!(
                "未知板块键 {}（可用：{}）",
                it.key,
                HOME_SECTION_KEYS.join("/")
            )));
        }
        if !seen.insert(&it.key) {
            return Err(DomainError::Validation(format!("板块 {} 重复", it.key)));
        }
        if ![0, 1, 2, 3].contains(&it.span) {
            return Err(DomainError::Validation(
                "span 取值 1/2/3（缺省自动）".into(),
            ));
        }
        norm.push(serde_json::json!({ "key": it.key, "span": it.span }));
    }
    let value = serde_json::to_string(&norm).map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "INSERT INTO site_settings (name, value, descr, grp) VALUES ('home_layout', $1, '首页板块排版（JSON 数组，空 = 默认布局）', 'main')          ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now()",
    )
    .bind(&value)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    state
        .repo
        .audit(Some(auth.id), "home_layout.update", None)
        .await;
    Ok(ok(
        serde_json::json!({ "saved": norm.len(), "layout": norm }),
    ))
}
