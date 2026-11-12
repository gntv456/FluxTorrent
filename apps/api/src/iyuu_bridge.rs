//! IYUU 辅种面板（读侧）：详情页问「这个种在别站也有吗」。
//!
//! **本模块不发任何出站请求**。查重由 worker 的 `iyuu_refresh` job 每小时
//! 批量跑、写 `iyuu_crossseed` 表；这里只读表 + 拼展示。原因见该 job 的
//! 模块文档：IYUU 查重是配额制，详情页是全站最热页面，实时查两头都受不了。
//!
//! 与 `/api/v1/crossseed/*` 方向相反——那是第三方来问**我们**，这里是
//! **我们**去问 IYUU。关掉本模块不影响任何第三方来查本站。
//!
//! 站点名靠 `sid` 查表（查重接口只回 id）。表有三层，优先级从高到低：
//!   1. 站长手填的「站点表覆盖」（设置项，改完即生效）；
//!   2. worker 自动同步的站点表（Redis `iyuu:site_table`，24h）；
//!   3. 代码内置的 98 条表（冷启动/断网兜底）。

use std::collections::HashMap;

use serde::Serialize;

use crate::state::AppState;

mod sites;

/// worker 写入站点表的键（两边必须一致）。
const SITES_KEY: &str = "iyuu:site_table";

/// 面板配置。
#[derive(Clone, Debug)]
pub struct Conf {
    /// 站长开关；false 时连表都不读。
    pub enabled: bool,
    /// 站点在 IYUU 的 appid。
    pub appid: String,
    /// IYUU 签发的 secret。
    pub secret: String,
    /// 站点表覆盖（每行 `sid|名称|域名`）；优先级最高。
    pub sites_extra: String,
}

impl Conf {
    /// 开关打开且凭据齐了，表里才有数据可读。
    pub fn ready(&self) -> bool {
        self.enabled && !self.appid.is_empty() && !self.secret.is_empty()
    }
}

/// conf 的 60s 进程内缓存（ModuleFlags 模式）。
///
/// 详情页面板是全站最热路径之一，每次点开都打两行 site_settings 点查
/// （conf + hits）。TTL 60s：站长改配置最坏 1 分钟生效，可接受——
/// 设置保存处没有对应的 invalidate 钩子，全靠 TTL 兜底。
static CONF_CACHE: std::sync::OnceLock<
    tokio::sync::RwLock<Option<(Conf, std::time::Instant)>>,
> = std::sync::OnceLock::new();

fn conf_cache(
) -> &'static tokio::sync::RwLock<Option<(Conf, std::time::Instant)>> {
    CONF_CACHE.get_or_init(|| tokio::sync::RwLock::new(None))
}

/// 主动失效：设置保存口若涉及 iyuu_bridge_* 键可调用（当前无钩子，预留）。
#[allow(dead_code)]
pub async fn invalidate_conf_cache() {
    *conf_cache().write().await = None;
}

/// 一条辅种命中（面板上的一行）。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Hit {
    /// IYUU 站点 id（前端拿它取图标）
    pub sid: i64,
    /// 站点名；sid 三层表都没有时回落 `#<sid>`
    pub site: String,
    /// 站点主域名（未知站点为空串）
    pub domain: String,
    /// 对方站点里的种子 id
    pub torrent_id: i64,
    /// 对方站点该种的详情页；域名/模板缺失时为 None
    pub url: Option<String>,
}

/// 读面板配置。读不到（迁移未跑）按「未启用」处理，不报错。
///
/// 走 60s 进程内缓存（见 `CONF_CACHE`）——面板是热路径，省两行点查。
pub async fn conf(state: &AppState) -> Conf {
    {
        let guard = conf_cache().read().await;
        if let Some((c, at)) = guard.as_ref() {
            if at.elapsed() < std::time::Duration::from_secs(60) {
                return c.clone();
            }
        }
    }
    let fresh = load_conf(state).await;
    *conf_cache().write().await =
        Some((fresh.clone(), std::time::Instant::now()));
    fresh
}

/// 真正打库的读配置（缓存 miss 时走）。
async fn load_conf(state: &AppState) -> Conf {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT name, value FROM site_settings WHERE name = ANY($1)",
    )
    .bind(&[
        "iyuu_bridge_enabled".to_string(),
        "iyuu_bridge_appid".to_string(),
        "iyuu_bridge_secret".to_string(),
        "iyuu_bridge_sites_extra".to_string(),
    ])
    .fetch_all(&state.repo.read_db)
    .await
    .unwrap_or_default();
    let mut enabled = false;
    let mut appid = String::new();
    let mut secret = String::new();
    let mut sites_extra = String::new();
    for (name, value) in rows {
        let v = value.trim().to_string();
        match name.as_str() {
            "iyuu_bridge_enabled" => {
                enabled = v.eq_ignore_ascii_case("yes") || v == "1"
            }
            "iyuu_bridge_appid" => appid = v,
            "iyuu_bridge_secret" => secret = v,
            "iyuu_bridge_sites_extra" => sites_extra = v,
            _ => {}
        }
    }
    Conf {
        enabled,
        appid,
        secret,
        sites_extra,
    }
}

/// 读某个种子缓存的命中。**零出站**。
///
/// 表里没这行（还没轮到刷新）就当没有——详情页不能为此等待或报错。
pub async fn hits_of(
    state: &AppState,
    conf: &Conf,
    torrent_id: i64,
) -> Vec<Hit> {
    if !conf.ready() {
        return Vec::new();
    }
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT hits FROM iyuu_crossseed WHERE torrent_id = $1")
            .bind(torrent_id)
            .fetch_optional(&state.repo.read_db)
            .await
            .unwrap_or(None);
    let Some((hits,)) = row else {
        return Vec::new();
    };
    let parsed: Vec<flux_shared::iyuu::Hit> =
        serde_json::from_value(hits).unwrap_or_default();
    if parsed.is_empty() {
        return Vec::new();
    }
    let extra = parse_extra(&conf.sites_extra);
    let synced = site_table(state).await;
    parsed
        .into_iter()
        // 自站命中防御性过滤：本站已对外提供 IYUU 站点自描述
        // （/crossseed/iyuu/site），IYUU 中央的索引完全可能把我们也回成
        // 命中——「FluxTorrent 在 FluxTorrent 有辅种」没有信息量且刺眼。
        // 按域名比对（自站域名取 PUBLIC_SITE_URL 系环境变量，mailer 同源），
        // 解析不出域名时不过滤（宁可多显示也不误杀真命中）。
        .filter(|h| !is_self_hit(&extra, &synced, h.sid))
        .map(|h| {
            match resolve_site(&extra, &synced, h.sid) {
                Some(s) => {
                    let url = s.detail_url(h.torrent_id);
                    Hit {
                        sid: h.sid,
                        site: s.name,
                        domain: s.host,
                        torrent_id: h.torrent_id,
                        url,
                    }
                }
                // 三层表都没有：给出 id 而不是编个名字。
                None => Hit {
                    sid: h.sid,
                    site: format!("#{}", h.sid),
                    domain: String::new(),
                    torrent_id: h.torrent_id,
                    url: None,
                },
            }
        })
        .collect()
}

/// 站点表覆盖：每行 `sid|名称|域名`，`#` 开头为注释，坏行跳过。
///
/// 只做最宽松的解析——站长手写的表不该因为少个空格就整段失效。
fn parse_extra(text: &str) -> HashMap<i64, (String, String)> {
    let mut out = HashMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('|').map(str::trim);
        let Some(sid) = parts.next().and_then(|s| s.parse::<i64>().ok()) else {
            continue;
        };
        let Some(name) = parts.next().filter(|s| !s.is_empty()) else {
            continue;
        };
        let domain = parts.next().unwrap_or("").trim_end_matches('/');
        out.insert(sid, (name.to_string(), domain.to_string()));
    }
    out
}

/// 站点解析：站长覆盖 → worker 同步表 → 内置表。
fn resolve_site(
    extra: &HashMap<i64, (String, String)>,
    synced: &HashMap<i64, flux_shared::iyuu::Site>,
    sid: i64,
) -> Option<flux_shared::iyuu::Site> {
    if let Some((n, d)) = extra.get(&sid) {
        return Some(flux_shared::iyuu::Site::plain(n, d));
    }
    if let Some(s) = synced.get(&sid) {
        return Some(s.clone());
    }
    sites::by_sid(sid).map(|(n, d)| flux_shared::iyuu::Site::plain(n, d))
}

/// 解析某个 sid 的站点域名（图标代理用：只需要 host）。
///
/// 与 `hits_of` 共用 `resolve_site`，保证站长覆盖表对图标也生效；
/// 空域名直接当查不到，别拼出 `https:///favicon.ico`。
pub async fn host_of(
    state: &AppState,
    conf: &Conf,
    sid: i64,
) -> Option<String> {
    let extra = parse_extra(&conf.sites_extra);
    let synced = site_table(state).await;
    let host = resolve_site(&extra, &synced, sid)?.host;
    // 与 `Site::detail_url` 的 M-Team 特例保持一致：接口域名没有图标。
    let host = if host == "api.m-team.cc" {
        "kp.m-team.cc".to_string()
    } else {
        host
    };
    (!host.is_empty()).then_some(host)
}

/// 读 worker 写好的站点表。读不到返回空表（走内置兜底）。
async fn site_table(state: &AppState) -> HashMap<i64, flux_shared::iyuu::Site> {
    use redis::AsyncCommands;
    let mut c = state.redis.clone();
    let raw: Option<String> = c.get(SITES_KEY).await.ok().flatten();
    raw.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// 该 sid 是否本站自己：把站点域名与 PUBLIC_SITE_URL 系环境变量比 host。
///
/// 拿不到自站域名（没配 env）或三层表都解析不出该 sid 时返回 false
/// （不过滤）——宁可多显示一条自站，也不误杀真命中。
fn is_self_hit(
    extra: &HashMap<i64, (String, String)>,
    synced: &HashMap<i64, flux_shared::iyuu::Site>,
    sid: i64,
) -> bool {
    let Some(self_host) = self_host() else {
        return false;
    };
    let Some(site) = resolve_site(extra, synced, sid) else {
        return false;
    };
    // 统一口径：去协议、去尾斜杠、小写后整比（本站对外只有一个域名）。
    host_of_url(&site.host) == Some(self_host)
}

/// 本站对外 host：PUBLIC_SITE_URL / PUBLIC_WEB_URL / PUBLIC_API_URL
/// 逐个取（mailer::site_base 同源口径），解析出 host 小写返回。
fn self_host() -> Option<String> {
    ["PUBLIC_SITE_URL", "PUBLIC_WEB_URL", "PUBLIC_API_URL"]
        .iter()
        .find_map(|k| std::env::var(k).ok())
        .and_then(|v| host_of_url(&v))
}

/// 从「可能是完整 URL 也可能是裸域名」的串里抠 host，小写返回。
fn host_of_url(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    let bare = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let host = bare.split('/').next().unwrap_or("");
    // 去掉可能带的 :port（IYUU 站点表 base_url 一般不带，env 可能带）
    let host = host.split(':').next().unwrap_or("");
    let host = host.trim().to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// 新种发布后立即查这一条（fire-and-forget）。
///
/// 与 worker 的小时批互补：批量负责补齐历史，这个钩子保证**刚发布的种
/// 立刻**有辅种数据，不用等最多一小时。只查这一条，不占批量额度。
///
/// 三条纪律：不 await（发布不等网络）、失败只记日志（发布结果不受影响）、
/// 未启用/凭据不全直接返回（零出站）。
///
/// 参数收 `Arc<AppState>` 而非 `web::Data`：调用方除 actix handler（发种口）
/// 外还有服务层用例（审核过审 `decide_one`），两边都能 `.get_ref().clone()`
/// 或直接给 Arc。
pub fn spawn_refresh_one(state: std::sync::Arc<AppState>, torrent_id: i64) {
    tokio::spawn(async move {
        if let Err(e) = refresh_one(&state, torrent_id).await {
            tracing::warn!(torrent_id, error = %e, "IYUU 新种即时查重失败");
        }
    });
}

/// 查一条并落库。只在种已审核可见时才查（否则面板也不会显示）。
async fn refresh_one(state: &AppState, torrent_id: i64) -> Result<(), String> {
    let conf = conf(state).await;
    if !conf.ready() {
        return Ok(());
    }
    let info_hash: Option<String> = sqlx::query_scalar(
        "SELECT COALESCE(info_hash, '') FROM torrents \
         WHERE id = $1 AND approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| e.to_string())?;
    let Some(info_hash) = info_hash else {
        // 还在待审：等审核通过后的批次刷，或审核入口再触发。
        return Ok(());
    };
    if !flux_shared::iyuu::is_hex40(&info_hash) {
        return Ok(());
    }
    let cred =
        flux_shared::iyuu::Cred::new(conf.appid.clone(), conf.secret.clone());
    match flux_shared::iyuu::search(&cred, &info_hash).await {
        Some(hits) => {
            let db = &state.repo.db;
            let _ = flux_shared::iyuu::upsert_hits(db, torrent_id, &hits).await;
        }
        None => {
            let _ =
                flux_shared::iyuu::upsert_failed(&state.repo.db, torrent_id)
                    .await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
