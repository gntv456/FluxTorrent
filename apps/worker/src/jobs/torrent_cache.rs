//! 种子查询进程内缓存（0477）：info_hash → (id, size) 物理不变量。
//!
//! 13 万 peer 下 worker 每事件查一次 torrents（43 次/秒）是 PG 第一大耗时。
//! 同 hash 的 id/size 永不变化（删除走回收站，行仍在），长 TTL 缓存：
//! · 正缓存 1h；负缓存（不存在）10 分钟——新种发布后最多 10 分钟可被做种
//! · 负区单独限容 3 万：外部种子 announce 经 whitelist fail-open 放行，
//!   hash 种类百万级，不能让它们挤掉正区热条目
//!
//! 只缓存 (id, size) 两列；seeders/leechers 等易变列不进缓存。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

type Entry = (i64, i64); // (torrent_id, size)

struct CacheState {
    /// 正缓存：hash → ((id, size), put 时刻)。(id, size) 是物理不变量，
    /// TTL 1h（种子体积编辑的陈旧窗可接受——计费锚定在 snatches）。
    pos: HashMap<String, (Entry, Instant)>,
    /// 负缓存：hash → put 时刻。外部种子（用户客户端里挂的其它站资源）
    /// announce 会被 whitelist fail-open 放行成事件——这类 hash 种类极多
    /// （百万级），单独限容 3 万，防止把正缓存挤爆触发整体 clear。
    neg: HashMap<String, Instant>,
}

const POS_TTL: Duration = Duration::from_secs(3600);
const NEG_TTL: Duration = Duration::from_secs(600);
const NEG_CAP: usize = 30_000;

static CACHE: Mutex<Option<CacheState>> = Mutex::new(None);

fn with_cache<T>(f: impl FnOnce(&mut CacheState) -> T) -> T {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert(CacheState {
        pos: HashMap::new(),
        neg: HashMap::new(),
    });
    // 负缓存超容：只清负区（正区由真实种子数自然有界）
    if st.neg.len() > NEG_CAP {
        st.neg.clear();
    }
    f(st)
}

fn expired(at: Instant, ttl: Duration) -> bool {
    at.elapsed() >= ttl
}

/// 命中返回 Some(Some((id, size)))；负缓存返回 Some(None)；未命中 None
pub(super) fn get(hash: &str) -> Option<Option<Entry>> {
    with_cache(|st| {
        // 返回值：Some(Some(entry))=正缓存命中 / Some(None)=负缓存命中 / None=未命中
        if let Some((entry, at)) = st.pos.get(hash) {
            if !expired(*at, POS_TTL) {
                return Some(Some(*entry));
            }
            return None;
        }
        if let Some(at) = st.neg.get(hash) {
            if !expired(*at, NEG_TTL) {
                return Some(None);
            }
            return None;
        }
        None
    })
}

pub(super) fn put(hash: &str, entry: Option<Entry>) {
    with_cache(|st| match entry {
        Some(e) => {
            st.neg.remove(hash);
            st.pos.insert(hash.to_string(), (e, Instant::now()));
        }
        None => {
            st.pos.remove(hash);
            st.neg.insert(hash.to_string(), Instant::now());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip_and_negative() {
        put("abc123", Some((7, 12345)));
        assert_eq!(get("abc123"), Some(Some((7, 12345))));
        put("nope", None);
        assert_eq!(get("nope"), Some(None));
        assert_eq!(get("missing"), None);
    }

    #[test]
    fn positive_put_clears_negative() {
        put("k1", None);
        assert_eq!(get("k1"), Some(None));
        put("k1", Some((1, 2)));
        assert_eq!(get("k1"), Some(Some((1, 2))));
    }
}
