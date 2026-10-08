//! 未注册 info_hash 的负缓存（审计 10-07 P3 自 guard_store.rs 拆出，
//! 五轮 300 行门禁）。清理时机与段封禁、swarm 元数据不同：本缓存在每次
//! guard 刷新时整体清空 ⇒ 新发种最迟一个刷新周期就能 announce。

use std::sync::{OnceLock, RwLock};

/// 未注册 info_hash 负缓存（审计 10-07 P3）：旧版每次 miss 都直查 PG，
/// 随机 hash 洪水 = 一请求一查询，白名单自己成了 DB 放大器。
/// 60s 且每次 guard 刷新即清 ⇒ 新发种最迟一个刷新周期可 announce。
pub(crate) fn hash_miss() -> &'static RwLock<std::collections::HashSet<String>>
{
    static V: OnceLock<RwLock<std::collections::HashSet<String>>> =
        OnceLock::new();
    V.get_or_init(|| RwLock::new(std::collections::HashSet::new()))
}

pub(crate) fn miss_seen(info_hash: &str) -> bool {
    match hash_miss().read() {
        Ok(r) => r.contains(info_hash),
        Err(e) => e.into_inner().contains(info_hash),
    }
}

pub(crate) fn remember_miss(info_hash: &str) {
    if let Ok(mut w) = hash_miss().write() {
        if w.len() > 200_000 {
            w.clear(); // 洪水兜底：宁可重新查一轮
        }
        w.insert(info_hash.to_string());
    }
}

pub(crate) fn clear_miss() {
    if let Ok(mut w) = hash_miss().write() {
        w.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 负缓存表是进程内静态，cargo 默认并行跑测试 ⇒ 碰它的用例必须串行
    fn lock() -> std::sync::MutexGuard<'static, ()> {
        static L: std::sync::OnceLock<std::sync::Mutex<()>> =
            std::sync::OnceLock::new();
        L.get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn miss_cache_roundtrip_and_clear() {
        let _g = lock();
        let h = "ff".repeat(20);
        assert!(!miss_seen(&h));
        remember_miss(&h);
        assert!(miss_seen(&h));
        clear_miss();
        assert!(!miss_seen(&h));
    }
}
