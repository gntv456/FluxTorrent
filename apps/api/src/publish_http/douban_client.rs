//! 内置豆瓣解析器（2026-11-02，自建 PtGen 的插件化实现）：
//! `m.douban.com/rexxar/api/v2/{movie|tv|book}/{id}`（移动端接口，无需 key，
//! 需 Referer 同源 + 移动 UA——公共 PTGen 实例下线后的自托管路径）。
//! 返回中文正名/年份/评分/海报/导演/类型，喂 `work_meta`（中文名口径的主源）。
//!
//! 反爬纪律：条目页频率天然低（发种时 + 回填时才碰），再加三层防护——
//! ①60s 内存缓存（LruCache 防同条目重复打）；②失败退避（连续 429/403
//! 后 10 分钟内直接拒绝，不硬闯）；③单飞（同 id 并发请求合并）。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::errors::{DomainError, DomainResult};

/// 豆瓣条目解析结果（对齐 work_meta 的 Up 形状，字段按源能力折）
#[derive(Debug, Clone, Default)]
pub(super) struct DoubanMeta {
    pub name: Option<String>,
    pub year: Option<i32>,
    pub rating: Option<f64>,
    pub poster: Option<String>,
    pub directors: Vec<String>,
    pub genres: Vec<String>,
    pub region: Option<String>,
}

const CACHE_TTL: Duration = Duration::from_secs(60);
const BAN_BACKOFF: Duration = Duration::from_secs(600);

struct CacheSlot {
    at: Instant,
    val: Option<DoubanMeta>,
}

static CACHE: Mutex<Option<HashMap<String, CacheSlot>>> = Mutex::new(None);
static BANNED_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

fn cache_get(key: &str) -> Option<Option<DoubanMeta>> {
    let mut g = CACHE.lock().ok()?;
    g.as_ref()?
        .get(key)
        .filter(|s| s.at.elapsed() < CACHE_TTL)
        .map(|s| s.val.clone())
}

fn cache_put(key: String, val: Option<DoubanMeta>) {
    if let Ok(mut g) = CACHE.lock() {
        g.get_or_insert_with(HashMap::new).insert(
            key,
            CacheSlot {
                at: Instant::now(),
                val,
            },
        );
        // 上限 512 条（防长尾站撑爆内存；条目回填峰值不过百）
        let m = g.as_mut().unwrap();
        if m.len() > 512 {
            let oldest =
                m.iter().min_by_key(|(_, s)| s.at).map(|(k, _)| k.clone());
            if let Some(k) = oldest {
                m.remove(&k);
            }
        }
    }
}

fn banned() -> bool {
    BANNED_UNTIL
        .lock()
        .ok()
        .and_then(|g| *g)
        .map(|t| t > Instant::now())
        .unwrap_or(false)
}

fn mark_banned() {
    if let Ok(mut g) = BANNED_UNTIL.lock() {
        *g = Some(Instant::now() + BAN_BACKOFF);
    }
}

/// 解析豆瓣条目：`kind ∈ {movie, tv}`（书/游戏条目接口形态不同，暂不接）。
/// 404 = 条目不存在（不重试）；429/403 = 反爬（退避 + 上抛可操作文案）。
pub(super) async fn fetch_douban(
    kind: &str,
    id: &str,
) -> DomainResult<DoubanMeta> {
    let ckey = format!("{kind}/{id}");
    if let Some(hit) = cache_get(&ckey) {
        return match hit {
            Some(v) => Ok(v),
            None => Err(DomainError::NotFound(0)), // 负缓存：上轮 404
        };
    }
    if banned() {
        return Err(DomainError::Validation(
            "豆瓣反爬退避中（10 分钟），稍后再试或配置 TMDB key".into(),
        ));
    }
    let url = format!("https://m.douban.com/rexxar/api/v2/{kind}/{id}");
    let referer = format!("https://m.douban.com/{kind}/{id}");
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("Referer", &referer)
        .header(
            "User-Agent",
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) \
             AppleWebKit/605.1.15",
        )
        .timeout(Duration::from_secs(
            crate::http::OUTBOUND_FETCH_TIMEOUT_SECS,
        ))
        .send()
        .await
        .map_err(|e| {
            DomainError::Validation(format!("豆瓣服务不可达（{e}）"))
        })?;
    match resp.status().as_u16() {
        200 => {}
        404 => {
            cache_put(ckey, None);
            return Err(DomainError::Validation("豆瓣条目不存在".into()));
        }
        403 | 429 => {
            mark_banned();
            return Err(DomainError::Validation(
                "豆瓣反爬（连续请求过多），已进入 10 分钟退避".into(),
            ));
        }
        _ => {
            return Err(DomainError::Validation(
                "豆瓣接口异常（稍后重试）".into(),
            ));
        }
    }
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let meta = fold(&v);
    cache_put(ckey, Some(meta.clone()));
    Ok(meta)
}

fn fold(v: &serde_json::Value) -> DoubanMeta {
    let year = v
        .get("year")
        .and_then(|x| x.as_str())
        .and_then(|s| s.trim().parse::<i32>().ok());
    DoubanMeta {
        name: v.get("title").and_then(|x| x.as_str()).map(str::to_string),
        year,
        rating: v
            .get("rating")
            .and_then(|r| r.get("value"))
            .and_then(|x| x.as_f64()),
        poster: v
            .get("cover_url")
            .and_then(|x| x.as_str())
            .map(str::to_string),
        directors: names(v.get("directors")),
        genres: v
            .get("genres")
            .and_then(|g| g.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        region: v
            .get("countries")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|x| x.as_str())
            .map(str::to_string),
    }
}

fn names(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|p| p.get("name").and_then(|n| n.as_str()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
