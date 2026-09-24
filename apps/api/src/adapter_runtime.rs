//! 适配器运行时（生态商店 M4，策划案 §5.2–5.4）：wasmtime 进程内沙箱。
//!
//! 职责与边界（冻结面 v1，R2：扩面需品类级评审）：
//! - guest 只暴露一个导出 `fetch(url: str) -> str`（JSON 结果）；health 由
//!   fetch("") 空参调用代替（M4 从简，v2 再拆 health_check）；
//! - 宿主 API 四函数（wasm 侧绑定，线性内存边界由 wasmtime 保证）：
//!     host_http_fetch(url, body_json) -> json  —— 出网白名单 + 20s 超时 + 限流
//!     host_cache_get(key) / host_cache_set(key, val, ttl_s) —— 适配器命名空间 KV
//!       （进程内 LRU，M4 不落 Redis：元数据 TTL 小、重启可接受）
//!     host_log(level, msg) —— tracing 透传（前缀 adapter_id）
//! - epoch 限额（死循环防线）：wasmtime epoch_interruption + tokio 定时 bump；
//! - 熔断（A4）：调用方（adapter_http）按 Err 维护 adapters.strikes，≥3 自动停用；
//!   本模块只负责「一次调用的成败与错误分类」。
//!
//! 明确不提供：SQL、文件、环境变量、响应改写、请求 veto（不变式 2）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wasmtime::component::Component;
use wasmtime::{Config, Engine, Linker, Store};

use crate::errors::{DomainError, DomainResult};

/// 死循环防线：epoch 预算（调用开始到 bump 的时间窗）
const EPOCH_TIMEOUT: Duration = Duration::from_secs(10);
/// 单次适配器调用的挂钟上限（含 http_fetch 在内的总时长）
const CALL_TIMEOUT: Duration = Duration::from_secs(25);
/// 适配器进程内缓存容量（条数）
const CACHE_CAP: usize = 512;

/// 适配器清单（行记录的内存形状；DB 行 + 运行态共用）
pub(crate) struct AdapterManifest {
    pub adapter_id: String,
    pub kind: String,
    pub http_allow: Vec<String>,
    pub secrets_read: Vec<String>,
    pub rate_limit_per_min: i32,
}

/// 一次调用的上下文（宿主状态）
struct HostState {
    manifest: AdapterManifest,
    /// 进程内 KV（key 前缀 adapter_id 隔离）
    cache: Arc<Mutex<HashMap<String, (String, Instant)>>>,
    /// 限流：窗口起点 + 计数
    rate: Arc<Mutex<(Instant, u64)>>,
    /// 出网执行器（async 由 wrapper 线程承接，见 host_http_fetch 注释）
    http: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
}

impl HostState {
    fn cache_get(&self, key: &str) -> Option<String> {
        let map = self.cache.lock().ok()?;
        let (v, exp) = map.get(&format!("{}:{}", self.manifest.adapter_id, key))?.clone();
        (Instant::now() < exp).then_some(v)
    }
    fn cache_set(&self, key: &str, val: &str, ttl_s: u64) {
        if let Ok(mut map) = self.cache.lock() {
            if map.len() >= CACHE_CAP {
                map.retain(|_, (_, exp)| Instant::now() < *exp);
            }
            map.insert(
                format!("{}:{}", self.manifest.adapter_id, key),
                (val.to_string(), Instant::now() + Duration::from_secs(ttl_s.max(1))),
            );
        }
    }
    fn rate_take(&self) -> bool {
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

/// 一次性引擎（每模块每调用实例化；元数据调用频率低，M4 不做实例池——
/// wasmtime Engine 构造重，进程内以 OnceLock 共享）
pub(crate) struct AdapterRuntime {
    pub engine: Engine,
}

impl AdapterRuntime {
    pub fn global() -> &'static AdapterRuntime {
        static RT: std::sync::OnceLock<AdapterRuntime> =
            std::sync::OnceLock::new();
        RT.get_or_init(|| {
            let mut cfg = Config::new();
            cfg.epoch_interruption(true);
            cfg.wasm_component_model(false);
            // crash-if-hung 不开（win 兼容）；epoch 由调用方 bump 线程驱动
            let engine = Engine::new(&cfg)
                .expect("wasmtime engine init");
            AdapterRuntime { engine }
        })
    }

    /// 出网白名单匹配（glob 近似：前缀 + `**` 尾通配）
    /// 出网白名单匹配（glob 近似）：
    /// `https://example.com/**` → 该 host 任意路径；
    /// `https://*.douban.com/**` → 子域（含裸域）任意路径；
    /// 无 `/**` 后缀 → 精确 URL。全部强制 https。
    fn url_allowed(manifest: &AdapterManifest, url: &str) -> bool {
        let u = url.trim();
        let https = u.starts_with("https://");
        let host_path = u.strip_prefix("https://").unwrap_or("");
        manifest.http_allow.iter().any(|pat| {
            let Some(base) = pat.strip_prefix("https://") else {
                return u == *pat && https;
            };
            let Some(base) = base.strip_suffix("/**") else {
                return u == *pat && https;
            };
            if !https || host_path.is_empty() {
                return false;
            }
            if let Some(prefix) = base.strip_prefix("*.") {
                // *.douban.com：裸域或任意子域
                let host = host_path.split('/').next().unwrap_or("");
                host == prefix || host.ends_with(&format!(".{prefix}"))
            } else {
                host_path.starts_with(base)
            }
        })
    }

    /// 一次调用：实例化 → epoch 预算 → fetch 导出 → JSON 结果。
    /// http 执行器由调用方注入（测试可替换）。
    pub(crate) fn call(
        &self,
        manifest: &AdapterManifest,
        wasm_bytes: &[u8],
        url: &str,
        http: Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync>,
    ) -> DomainResult<String> {
        let module = wasmtime::Module::new(&self.engine, wasm_bytes)
            .map_err(|e| adapter_err(manifest, &format!("wasm 编译失败: {e}")))?;
        let mut store = Store::new(
            &self.engine,
            HostState {
                manifest: AdapterManifest {
                    adapter_id: manifest.adapter_id.clone(),
                    kind: manifest.kind.clone(),
                    http_allow: manifest.http_allow.clone(),
                    secrets_read: manifest.secrets_read.clone(),
                    rate_limit_per_min: manifest.rate_limit_per_min,
                },
                cache: Arc::new(Mutex::new(HashMap::new())),
                rate: Arc::new(Mutex::new((Instant::now(), 0))),
                http,
            },
        );
        store.set_epoch_deadline(1);
        // epoch bump 线程：EPOCH_TIMEOUT 后触发 trap
        let engine = self.engine.clone();
        let bump = std::thread::spawn(move || {
            std::thread::sleep(EPOCH_TIMEOUT);
            engine.increment_epoch();
        });

        let mut linker = Linker::new(&self.engine);
        linker
            .func_wrap(
                "host",
                "http_fetch",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 url_ptr: i32, url_len: i32,
                 out_ptr: i32, out_cap: i32|
                 -> i32 {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None => return -1,
                    };
                    let (data, store) = mem.data_and_store_mut(&mut caller);
                    let url = match read_str(data, url_ptr, url_len) {
                        Some(s) => s,
                        None => return -1,
                    };
                    if !AdapterRuntime::url_allowed(&store.manifest, &url) {
                        tracing::warn!(adapter = store.manifest.adapter_id.as_str(), %url, "出网越权拒绝");
                        return -2;
                    }
                    if !store.rate_take() {
                        return -3;
                    }
                    let body = match (store.http)(&url) {
                        Ok(b) => b,
                        Err(_) => return -4,
                    };
                    write_str(data, out_ptr, out_cap, &body)
                },
            )
            .and_then(|l| l.func_wrap(
                "host",
                "cache_get",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 key_ptr: i32, key_len: i32,
                 out_ptr: i32, out_cap: i32|
                 -> i32 {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None => return -1,
                    };
                    let (data, store) = mem.data_and_store_mut(&mut caller);
                    let key = match read_str(data, key_ptr, key_len) {
                        Some(s) => s,
                        None => return -1,
                    };
                    match store.cache_get(&key) {
                        Some(v) => write_str(data, out_ptr, out_cap, &v),
                        None => 0,
                    }
                },
            ))
            .and_then(|l| l.func_wrap(
                "host",
                "cache_set",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 key_ptr: i32, key_len: i32,
                 val_ptr: i32, val_len: i32,
                 ttl: i64| {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None => return,
                    };
                    let (data, store) = mem.data_and_store_mut(&mut caller);
                    if let (Some(k), Some(v)) = (
                        read_str(data, key_ptr, key_len),
                        read_str(data, val_ptr, val_len),
                    ) {
                        store.cache_set(&k, &v, ttl.max(0) as u64);
                    }
                },
            ))
            .and_then(|l| l.func_wrap(
                "host",
                "log",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 level: i32,
                 msg_ptr: i32, msg_len: i32| {
                    let mem = match caller.get_export("memory").and_then(|e| e.into_memory()) {
                        Some(m) => m,
                        None => return,
                    };
                    let (data, store) = mem.data_and_store_mut(&mut caller);
                    if let Some(m) = read_str(data, msg_ptr, msg_len) {
                        match level {
                            2 => tracing::warn!(adapter = store.manifest.adapter_id.as_str(), "{m}"),
                            _ => tracing::info!(adapter = store.manifest.adapter_id.as_str(), "{m}"),
                        }
                    }
                },
            ))
            .map_err(|e| DomainError::Internal(e.into()))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| adapter_err(manifest, &format!("wasm 实例化失败: {e}")))?;
        let fetch = instance
            .get_typed_func::<(i32, i32, i32), i32>(&mut store, "fetch")
            .map_err(|_| adapter_err(manifest, "缺少 fetch 导出"))?;
        // 入参写进线性内存（guest 导出的 PBR 布局：in_ptr/in_cap 固定 0/N）
        let mem = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| adapter_err(manifest, "缺少 memory 导出"))?;
        let url_bytes = url.as_bytes();
        let out_cap = 1 << 20; // 1MiB 结果缓冲
        let in_region = 64;
        let out_region = in_region + url_bytes.len().max(1);
        let total = out_region + out_cap;
        mem.grow(&mut store, ((total + 65535) / 65536) as u64)
            .map_err(|_| adapter_err(manifest, "内存增长失败"))?;
        mem.write(&mut store, in_region, url_bytes)
            .map_err(|e| DomainError::Internal(e.into()))?;

        let started = Instant::now();
        let write_len = fetch
            .call(&mut store, (in_region as i32, url_bytes.len() as i32, out_region as i32))
            .map_err(|e| adapter_err(manifest, &format!("wasm 执行失败: {e}")))?;
        let _ = bump.join();
        if started.elapsed() > CALL_TIMEOUT {
            return Err(adapter_err(manifest, "适配器调用超时"));
        }
        if write_len < 0 {
            return Err(adapter_err(
                manifest,
                match write_len {
                    -2 => "出网越权（白名单外 URL）",
                    -3 => "限流",
                    -4 => "上游请求失败",
                    _ => "宿主调用失败",
                },
            ));
        }
        let mut out = vec![0u8; write_len as usize];
        mem.read(&mut store, out_region, &mut out)
            .map_err(|e| DomainError::Internal(e.into()))?;
        String::from_utf8(out)
            .map_err(|_| adapter_err(manifest, "结果非 UTF-8"))
    }
}

fn adapter_err(m: &AdapterManifest, msg: &str) -> DomainError {
    DomainError::Validation(format!("适配器 {}: {}", m.adapter_id, msg))
}

fn read_str(data: &[u8], ptr: i32, len: i32) -> Option<String> {
    if ptr < 0 || len < 0 {
        return None;
    }
    let s = data.get(ptr as usize..(ptr + len) as usize)?;
    String::from_utf8(s.to_vec()).ok()
}

/// 返回写入长度（≥0）或 -1（容量不足）
fn write_str(data: &mut [u8], ptr: i32, cap: i32, s: &str) -> i32 {
    let b = s.as_bytes();
    if b.len() > cap as usize || ptr < 0 {
        return -1;
    }
    data[ptr as usize..ptr as usize + b.len()].copy_from_slice(b);
    b.len() as i32
}

// Component 模型 v2 预留（M4 用核心 wasm + 手写 ABI）
#[allow(dead_code)]
fn _component_placeholder(_: Component) {}
