//! IYUU 查重客户端（api 与 worker 共用）。
//!
//! 协议以**好学旧站实跑的那套**为准（NexusPHP `public/details.php` 的
//! `getIYUU_ReseedSites()`，2026-11-11 真机验证）：
//!
//! `POST https://2025.iyuu.cn/spider/reseed/index`
//! ```json
//! {"info_hash":"<hex40>","sign":"<sha1(timestamp+secret)>",
//!  "appid":"<站点 appid>","timestamp":1700000000,"version":"8.0.0"}
//! ```
//! 返回 `{code:200,data:[{hash,torrent:[{info_hash,sid,torrent_id}]}]}`。
//!
//! 站点表：`POST https://2025.iyuu.cn/spider/site/index`
//! 返回 `{code:200,data:{count,sites:[{id,site,nickname,base_url,
//! details_page,is_https,cookie_required}]}}`。
//! **注意是单数 `site`**——复数 `sites` 是 404，这条踩过。
//!
//! 为什么放共享 crate：查重要限量（配额制），不能在详情页每次点开就打一次
//! IYUU。真正的外呼由 worker 定时批量跑，api 只读库。两边必须用**同一份**
//! 签名与解析，否则口径会漂。
//!
//! **协议能力边界（2026-11-12 核实，勿重查）**：IYUU **只按 info_hash 匹配**，
//! 不收 pieces 指纹——别想着「补发 pieces_hash 命中换 source 重打包的同源
//! 种」。IYUUPlus 客户端 `ReseedServices.php` 里 reseed() 的第二个参数
//! `sha1` 是 **hash 列表的校验和**（`sha1(json_encode(info_hash数组))`，
//! 用于服务端校验列表完整性），不是 `sha1(info.pieces)`。spider 站点协议
//! （本模块走的）更是只有单数 `info_hash` 字段。跨站重打包的匹配面在
//! `/api/v1/crossseed/lookup` 的 `pieces_sha1`（第三方直连我们时才用得上）。

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

/// 查重接口。
pub const SEARCH_URL: &str = "https://2025.iyuu.cn/spider/reseed/index";
/// 站点表接口（单数 `site`）。
pub const SITES_URL: &str = "https://2025.iyuu.cn/spider/site/index";
/// 请求版本号，与旧站一致。
pub const API_VERSION: &str = "8.0.0";
/// 查重超时（秒）。2026-11-11 真机实测：IYUU 慢的时候 5 秒根本回不来，
/// 一半请求被误判成失败写进 `ok=false`。放宽到 10 秒，配额制不重试。
/// 2026-11-13 好学站首扫实测再放宽到 20：IYUU 上游双峰延迟——要么
/// <8s 回来，要么整段抖窗 >30s 全超时（约 20% 概率），10s 卡在两峰
/// 之间，连败 5 条熔断整轮白跑。20s 能吃下慢峰的大多数，剩下的真抖窗
/// 交给 consec-fail 熔断兜底（下轮再补），失败不丢数据只推 fetched_at。
const TIMEOUT_SECS: u64 = 20;
/// 站点表超时（秒）：这个接口要返回约 16KB，实测 12 秒才回全包；
/// 一天只拉一次，等久一点不划算的是「同步失败」，不是这几秒。
/// 2026-11-13 好学站首扫实测 25s 仍会撞抖窗（>30s 全超时段）——已带
/// 一次重试（`sites()` 两连试），彻底失败沿用内置 98 站表兜底。
const SITES_TIMEOUT_SECS: u64 = 25;

/// 站点凭据。
#[derive(Clone, Debug)]
pub struct Cred {
    pub appid: String,
    pub secret: String,
}

impl Cred {
    pub fn new(appid: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            appid: appid.into(),
            secret: secret.into(),
        }
    }

    /// 凭据齐了才允许出站。
    pub fn ready(&self) -> bool {
        !self.appid.trim().is_empty() && !self.secret.trim().is_empty()
    }
}

/// 一个站点在面板上需要的全部信息。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Site {
    /// 站点名（面板上显示的）
    pub name: String,
    /// 主域名，不含协议
    pub host: String,
    /// 详情页路径模板，`{}` 是种子 id
    pub details: String,
    /// 是否走 https（IYUU 的 `is_https`：0=http，1/2=https）
    pub https: bool,
}

impl Site {
    /// 只有名字和域名时，详情页按 NexusPHP 通用口径兜底
    /// （IYUU 125 个站里 112 个就是 `details.php?id={}`）。
    pub fn plain(name: &str, host: &str) -> Self {
        Self {
            name: name.to_string(),
            host: host.trim_end_matches('/').to_string(),
            details: "details.php?id={}".to_string(),
            https: true,
        }
    }

    /// 拼该站种子的详情页。模板或域名缺失时返回 None——宁可不给，
    /// 也不给一个点开是 404 的链接。
    pub fn detail_url(&self, torrent_id: i64) -> Option<String> {
        if self.host.is_empty() || self.details.is_empty() {
            return None;
        }
        // M-Team 的唯一特例：`base_url` 是接口域名，网页在 kp/。
        let host = if self.host == "api.m-team.cc" {
            "kp.m-team.cc"
        } else {
            self.host.as_str()
        };
        let scheme = if self.https { "https" } else { "http" };
        let path = self.details.replace("{}", &torrent_id.to_string());
        Some(format!(
            "{scheme}://{host}/{}",
            path.trim_start_matches('/')
        ))
    }
}

/// 一条命中：对方站点的 sid 与其种子 id。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub sid: i64,
    pub torrent_id: i64,
}

/// 签名：`sha1(timestamp + secret)`，与旧站 `calculateIYUU_Sign()` 同口径。
pub fn sign(secret: &str, ts: i64) -> String {
    let mut h = Sha1::new();
    h.update(format!("{ts}{secret}").as_bytes());
    hex_lower(&h.finalize())
}

/// 小写 hex。
pub fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// 40 位 hex 校验。形状不对就别出站（省一次必然失败的请求）。
pub fn is_hex40(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn client(timeout_secs: u64) -> Option<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .ok()
}

fn signed_body(cred: &Cred, ts: i64) -> serde_json::Value {
    serde_json::json!({
        "sign": sign(&cred.secret, ts),
        "appid": cred.appid,
        "timestamp": ts,
        "version": API_VERSION,
    })
}

/// 查一个 info_hash 在别站的同种。
///
/// 返回 `None` = 请求失败（网络/HTTP/解析），`Some(vec![])` = 查到了但没命中。
/// 两者对调用方的处理不同：失败不该被当成「这个种没人有」写进库。
pub async fn search(cred: &Cred, info_hash: &str) -> Option<Vec<Hit>> {
    if !cred.ready() || !is_hex40(info_hash) {
        return None;
    }
    let client = client(TIMEOUT_SECS)?;
    let ts = chrono::Utc::now().timestamp();
    let mut body = signed_body(cred, ts);
    body["info_hash"] = serde_json::Value::String(info_hash.to_string());
    let resp = client.post(SEARCH_URL).json(&body).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let payload = resp.json::<serde_json::Value>().await.ok()?;
    // `code != 200`：400 = 未命中（正常），其它按失败处理，
    // 免得把「配额用尽」误记成「没人有」。
    match payload.get("code").and_then(|c| c.as_i64()) {
        Some(200) => Some(parse_hits(&payload)),
        Some(400) => Some(Vec::new()),
        _ => None,
    }
}

/// 拉站点表。返回 `None` = 请求失败。
///
/// 这个接口慢且偶发超时，给两次机会；一天只拉一次，重试不占查重配额。
/// 两次都失败就沿用内置表，下一轮 job 会再试（Redis 里没写键）。
pub async fn sites(cred: &Cred) -> Option<HashMap<i64, Site>> {
    if !cred.ready() {
        return None;
    }
    for _ in 0..2 {
        if let Some(m) = sites_once(cred).await {
            return Some(m);
        }
    }
    None
}

async fn sites_once(cred: &Cred) -> Option<HashMap<i64, Site>> {
    let client = client(SITES_TIMEOUT_SECS)?;
    let ts = chrono::Utc::now().timestamp();
    let resp = client
        .post(SITES_URL)
        .json(&signed_body(cred, ts))
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let payload = resp.json::<serde_json::Value>().await.ok()?;
    let map = parse_sites(&payload);
    if map.is_empty() {
        return None;
    }
    Some(map)
}

/// 解析查重响应：`data[0].torrent[]`，按 sid 去重（面板一行一个站）。
pub fn parse_hits(payload: &serde_json::Value) -> Vec<Hit> {
    let Some(rows) = payload
        .get("data")
        .and_then(|d| d.as_array())
        .and_then(|a| a.first())
        .and_then(|d| d.get("torrent"))
        .and_then(|t| t.as_array())
    else {
        return Vec::new();
    };
    let mut seen: HashMap<i64, i64> = HashMap::new();
    for item in rows {
        let sid = item.get("sid").and_then(|x| x.as_i64());
        let tid = item.get("torrent_id").and_then(|x| x.as_i64());
        if let (Some(sid), Some(tid)) = (sid, tid) {
            seen.entry(sid).or_insert(tid);
        }
    }
    let mut out: Vec<Hit> = seen
        .into_iter()
        .map(|(sid, torrent_id)| Hit { sid, torrent_id })
        .collect();
    // 稳定输出：按 sid 升序，缓存串与前端顺序都不抖。
    out.sort_unstable_by_key(|h| h.sid);
    out
}

/// 解析站点表响应：`{code:200,data:{count,sites:[…]}}`。
pub fn parse_sites(payload: &serde_json::Value) -> HashMap<i64, Site> {
    let mut out = HashMap::new();
    if payload.get("code").and_then(|c| c.as_i64()) != Some(200) {
        return out;
    }
    let Some(rows) = payload
        .get("data")
        .and_then(|d| d.get("sites"))
        .and_then(|s| s.as_array())
    else {
        return out;
    };
    for it in rows {
        let Some(sid) = it.get("id").and_then(|x| x.as_i64()) else {
            continue;
        };
        let name = it
            .get("nickname")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .trim();
        let host = it
            .get("base_url")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .trim();
        if name.is_empty() || host.is_empty() {
            continue;
        }
        out.insert(
            sid,
            Site {
                name: name.to_string(),
                host: host.trim_end_matches('/').to_string(),
                details: it
                    .get("details_page")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
                https: it.get("is_https").and_then(|x| x.as_i64()) != Some(0),
            },
        );
    }
    out
}

/// 写入一次成功的查重结果（含「查成功但没人有」的空数组）。
///
/// 放共享 crate：api 的「新种即时查」和 worker 的批量刷新写同一张表，
/// 两处各写一份 SQL 迟早漂移（一个写 ok 一个忘写 fetched_at 那种）。
pub async fn upsert_hits(
    db: &sqlx::PgPool,
    torrent_id: i64,
    hits: &[Hit],
) -> sqlx::Result<()> {
    let payload = serde_json::to_value(hits).unwrap_or_default();
    sqlx::query(
        "INSERT INTO iyuu_crossseed (torrent_id, hits, ok, fetched_at) \
         VALUES ($1, $2, TRUE, now()) \
         ON CONFLICT (torrent_id) DO UPDATE \
           SET hits = EXCLUDED.hits, ok = TRUE, fetched_at = now()",
    )
    .bind(torrent_id)
    .bind(payload)
    .execute(db)
    .await?;
    Ok(())
}

/// 记一次失败：**保留上次的命中**（不清空），只把 `fetched_at` 推进，
/// 让下一轮先刷别的种子，而不是死磕同一条；也不至于一次配额用尽
/// 就让整块面板变空。
pub async fn upsert_failed(
    db: &sqlx::PgPool,
    torrent_id: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO iyuu_crossseed (torrent_id, hits, ok, fetched_at) \
         VALUES ($1, '[]'::jsonb, FALSE, now()) \
         ON CONFLICT (torrent_id) DO UPDATE \
           SET ok = FALSE, fetched_at = now()",
    )
    .bind(torrent_id)
    .execute(db)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
