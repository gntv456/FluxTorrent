//! 种子查询进程内缓存（0477）：info_hash → (id, size) 物理不变量。
//!
//! 13 万 peer × 4.4 种下，worker 每事件查一次 torrents（43 次/秒，contention
//! 下 785ms/次）是 PG 第一大耗时。同 hash 的 id/size 永不变化（删除走
//! approve_status/回收站，行仍在），可长 TTL 缓存：
//! · 命中即零 DB 往返；负缓存（不存在）60s——新种发布后最多 60s 可被做种
//! · 容量 12 万（> 全站种子数 2.3 万 × 5 余量），超容整体清空重建
//!
//! 只缓存 (id, size) 两列；seeders/leechers 等易变列不进缓存。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_secs(300);
const NEG_TTL: Duration = Duration::from_secs(60);
const CAP: usize = 120_000;

type Entry = (i64, i64); // (torrent_id, size)

struct CacheState {
    map: HashMap<String, (Option<Entry>, Instant)>,
}

static CACHE: Mutex<Option<CacheState>> = Mutex::new(None);

fn with_cache<T>(f: impl FnOnce(&mut CacheState) -> T) -> T {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let st = guard.get_or_insert(CacheState {
        map: HashMap::new(),
    });
    // 超容整体清空（最坏一次全量回源，代价可承受）
    if st.map.len() > CAP {
        st.map.clear();
    }
    f(st)
}

/// 命中返回 Some(Some((id, size)))；负缓存返回 Some(None)；未命中 None
pub(super) fn get(hash: &str) -> Option<Option<Entry>> {
    with_cache(|st| {
        st.map.get(hash).and_then(|(entry, at)| {
            let ttl = if entry.is_some() { TTL } else { NEG_TTL };
            (at.elapsed() < ttl).then(|| *entry)
        })
    })
}

pub(super) fn put(hash: &str, entry: Option<Entry>) {
    with_cache(|st| {
        let ttl = if entry.is_some() { TTL } else { NEG_TTL };
        st.map
            .insert(hash.to_string(), (entry, Instant::now() + ttl));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip_and_negative() {
        put("abc123", Some((7, 12345)));
        assert_eq!(get("abc123"), Some(Some((7, 12345))));
        // 负缓存
        put("nope", None);
        assert_eq!(get("nope"), Some(None));
        // 未命中
        assert_eq!(get("missing"), None);
    }
}
