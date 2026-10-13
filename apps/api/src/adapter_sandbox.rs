//! 适配器沙箱的**宿主状态与内存边界**（批四十九按域拆自
//! `adapter_runtime.rs`）。
//!
//! 拆这一刀的根因（P-188/P-189）：原先 `HostState` 与 `Store` 都在 `call()` 里
//! 就地构造，`cache`/`rate` 两个 `Arc` 每次调用新建 ⇒
//! - `rate_limit_per_min`（字段名与注释都写着「每分钟」）只统计**单次调用内**
//!   guest 出网次数，跨调用永不累计，后台填 1 也拦不住连点；
//! - `host_cache_*` 承诺的「进程内 LRU」写完即随 Store 丢弃，命中率恒 0，
//!   容量清理分支永不可达。
//! 现在两张表由 `AdapterRuntime` 持有、按 adapter_id 分桶注入本文件的
//! `new_store`，本文件只负责「一次调用的宿主形状」。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wasmtime::{Engine, Store};

use crate::adapter_runtime::AdapterManifest;

/// guest 线性内存 + 表上限（四轮审计 M2）：64MiB 足够元数据抓取类适配器
pub(crate) const GUEST_MEM_LIMIT: usize = 64 * 1024 * 1024;
/// 适配器进程内缓存容量（条数）
const CACHE_CAP: usize = 512;
/// 缓存表条数上限（适配器个位数，越界时按过期清理兜底）
const KV_TABLE_CAP: usize = 2048;

/// 进程内 KV 表（key 已含 adapter_id 前缀，见 [`HostState::cache_get`]）
pub(crate) type KvTable = Arc<Mutex<HashMap<String, (String, Instant)>>>;
/// 单个适配器的限流桶（窗口起点 + 计数）
pub(crate) type RateBucket = Arc<Mutex<(Instant, u64)>>;

/// wasmtime ResourceLimiter 的最小实现：只限总量，不追踪增长语义
struct MemLimiter {
    limit: usize,
}

impl wasmtime::ResourceLimiter for MemLimiter {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        Ok(desired <= self.limit)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        Ok(desired <= self.limit)
    }
}

/// 一次调用的上下文（宿主状态）。`cache`/`rate` 是**跨调用共享**的句柄，
/// 由 `AdapterRuntime` 创建并按 adapter_id 分桶注入（P-188/P-189 的修法本体）。
pub(crate) struct HostState {
    pub(crate) manifest: AdapterManifest,
    mem_limiter: MemLimiter,
    pub(crate) cache: KvTable,
    pub(crate) rate: RateBucket,
    pub(crate) http: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
    pub(crate) secrets: HashMap<String, String>,
}

impl HostState {
    pub(crate) fn cache_get(&self, key: &str) -> Option<String> {
        let map = self.cache.lock().ok()?;
        let (v, exp) = map
            .get(&format!("{}:{}", self.manifest.adapter_id, key))?
            .clone();
        (Instant::now() < exp).then_some(v)
    }
    pub(crate) fn cache_set(&self, key: &str, val: &str, ttl_s: u64) {
        if let Ok(mut map) = self.cache.lock() {
            if map.len() >= CACHE_CAP {
                map.retain(|_, (_, exp)| Instant::now() < *exp);
            }
            if map.len() >= KV_TABLE_CAP {
                // 全是活条目时的兜底（宁可丢缓存也不无界增长）
                map.clear();
            }
            map.insert(
                format!("{}:{}", self.manifest.adapter_id, key),
                (
                    val.to_string(),
                    Instant::now() + Duration::from_secs(ttl_s.max(1)),
                ),
            );
        }
    }
    pub(crate) fn rate_take(&self) -> bool {
        if let Ok(mut r) = self.rate.lock() {
            let (at, n) = *r;
            if at.elapsed() >= Duration::from_secs(60) {
                *r = (Instant::now(), 1);
                return true;
            }
            if n >= self.manifest.rate_limit_per_min as u64 {
                return false;
            }
            *r = (at, n + 1);
            return true;
        }
        false
    }
}

/// 构造一次调用的 Store：注入共享 KV/限流桶，并挂上内存硬限与 epoch 预算。
/// 限流与缓存的生命周期在 `AdapterRuntime`，不在这里（P-188/P-189）。
pub(crate) fn new_store(
    engine: &Engine,
    manifest: AdapterManifest,
    secrets: HashMap<String, String>,
    http: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
    cache: KvTable,
    rate: RateBucket,
) -> Store<HostState> {
    let mut store = Store::new(
        engine,
        HostState {
            manifest,
            mem_limiter: MemLimiter {
                limit: GUEST_MEM_LIMIT,
            },
            cache,
            rate,
            http,
            secrets,
        },
    );
    // 四轮审计 M2（2026-10-07）：guest 自行 memory.grow 无上限（wasmtime
    // 实例可达 4GiB）——沙箱在 API 进程内，必须 limiter 硬限。
    store.limiter(|state| &mut state.mem_limiter);
    store.set_epoch_deadline(1);
    store
}


/// 取 guest 线性内存与宿主状态（五个 host 导入同形，抽成一处）
pub(crate) fn guest_mem<'a>(
    caller: &'a mut wasmtime::Caller<'_, HostState>,
) -> Option<(&'a mut [u8], &'a mut HostState)> {
    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())?;
    Some(mem.data_and_store_mut(caller))
}

pub(crate) fn read_str(data: &[u8], ptr: i32, len: i32) -> Option<String> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let s = data.get(ptr as usize..(ptr + len) as usize)?;
    String::from_utf8(s.to_vec()).ok()
}

/// 返回写入长度（≥0）或 -1（容量不足）
pub(crate) fn write_str(data: &mut [u8], ptr: i32, cap: i32, s: &str) -> i32 {
    let b = s.as_bytes();
    if b.len() > cap as usize || ptr < 0 {
        return -1;
    }
    data[ptr as usize..ptr as usize + b.len()].copy_from_slice(b);
    b.len() as i32
}
