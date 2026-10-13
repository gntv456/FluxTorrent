//! 适配器运行时（生态商店 M4，策划案 §5.2–5.4）：wasmtime 进程内沙箱。
//!
//! 职责与边界（冻结面 v1，R2：扩面需品类级评审）：
//! - guest 只暴露一个导出 `fetch(url: str) -> str`（JSON 结果）；health 由
//!   fetch("") 空参调用代替（M4 从简，v2 再拆 health_check）；
//! - 宿主 API 在 `adapter_host.rs`（五个 wasm 侧导入）、沙箱件与内存边界在
//!   `adapter_sandbox.rs`：
//!     host_http_fetch(url, body_json) -> json  —— 出网白名单 + 20s 超时 + 限流
//!     host_cache_get(key) / host_cache_set(key, val, ttl_s) —— 适配器命名空间 KV
//!       （进程内 LRU，M4 不落 Redis：元数据 TTL 小、重启可接受）
//!     host_log(level, msg) —— tracing 透传（前缀 adapter_id）
//! - **KV 表与限流桶的生命周期在本类型（`AdapterRuntime`）：进程内共享、按
//!   adapter_id 分桶**（批四十九 P-188/P-189）。原先两者都在 `call()` 里每次
//!   新建，于是「每分钟」的限流只统计单次调用内 guest 出网几次（跨调用恒从零
//!   起），缓存写完即随 Store 丢弃（命中率恒 0、容量清理分支永不可达）；
//! - epoch 限额（死循环防线）：wasmtime epoch_interruption + tokio 定时 bump；
//! - 熔断（A4）：调用方（adapter_http）按 Err 维护 adapters.strikes，≥3 自动停用；
//!   本模块只负责「一次调用的成败与错误分类」。
//!
//! 明确不提供：SQL、文件、环境变量、响应改写、请求 veto（不变式 2）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use wasmtime::component::Component;
use wasmtime::{Config, Engine};

use crate::adapter_host::build_linker;
use crate::adapter_sandbox::{new_store, KvTable, RateBucket};
use crate::errors::{DomainError, DomainResult};

/// 死循环防线：epoch 预算（调用开始到 bump 的时间窗）
const EPOCH_TIMEOUT: Duration = Duration::from_secs(10);
/// 单次适配器调用的挂钟上限（含 http_fetch 在内的总时长）
const CALL_TIMEOUT: Duration = Duration::from_secs(25);
/// 限流桶条数上限（适配器总量个位数，越界清空兜底）
const BUCKET_CAP: usize = 64;

/// 适配器清单（行记录的内存形状；DB 行 + 运行态共用）
pub(crate) struct AdapterManifest {
    pub adapter_id: String,
    pub kind: String,
    pub http_allow: Vec<String>,
    pub secrets_read: Vec<String>,
    pub rate_limit_per_min: i32,
}

/// 一次性引擎（每模块每调用实例化；元数据调用频率低，M4 不做实例池——
/// wasmtime Engine 构造重，进程内以 OnceLock 共享）
pub(crate) struct AdapterRuntime {
    pub engine: Engine,
    /// 编译产物缓存（四轮 M2）：Module 可跨 Store 复用（Send+Sync）
    module_cache: Mutex<HashMap<(String, String), wasmtime::Module>>,
    /// 适配器命名空间 KV（跨调用共享，key 含 adapter_id 前缀）
    kv: KvTable,
    /// 每个适配器一只限流桶（P-188 的本体：桶必须在调用之间活着）
    buckets: Mutex<HashMap<String, RateBucket>>,
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
            let engine = Engine::new(&cfg).expect("wasmtime engine init");
            AdapterRuntime {
                engine,
                module_cache: Mutex::new(HashMap::new()),
                kv: Arc::new(Mutex::new(HashMap::new())),
                buckets: Mutex::new(HashMap::new()),
            }
        })
    }

    /// 取（或建）该适配器的限流桶；窗口 60s 由 `rate_take` 自己滚。
    fn bucket_for(&self, adapter_id: &str) -> RateBucket {
        let mut map = self.buckets.lock().unwrap();
        if let Some(b) = map.get(adapter_id) {
            return b.clone();
        }
        if map.len() >= BUCKET_CAP {
            map.clear();
        }
        let b: RateBucket = Arc::new(Mutex::new((Instant::now(), 0)));
        map.insert(adapter_id.to_string(), b.clone());
        b
    }

    /// 四轮审计 M2（2026-10-07）：Module 按 (adapter_id, wasm 指纹) 缓存——
    /// 旧版每调用重编译 2MiB wasm，/ptgen 只需登录即可循环触发全量适配器
    /// 重编译打满 CPU。指纹取 wasm sha3 前 16 hex，同 id 换内容自动失效。
    fn module_cached(
        &self,
        manifest: &AdapterManifest,
        wasm_bytes: &[u8],
    ) -> DomainResult<wasmtime::Module> {
        use sha3::Digest;
        let mut h = sha3::Sha3_256::new();
        h.update(wasm_bytes);
        let d = h.finalize();
        let fp: String =
            d.iter().map(|b| format!("{b:02x}")).collect::<String>()[..16]
                .to_string();
        let key = (manifest.adapter_id.clone(), fp);
        if let Some(m) = self.module_cache.lock().unwrap().get(&key) {
            return Ok(m.clone());
        }
        let module =
            wasmtime::Module::new(&self.engine, wasm_bytes).map_err(|e| {
                adapter_err(manifest, &format!("wasm 编译失败: {e}"))
            })?;
        // 上限 32 条防无限增长（适配器总量个位数，32 是极宽裕的界）
        let mut c = self.module_cache.lock().unwrap();
        if c.len() >= 32 {
            c.clear();
        }
        c.insert(key, module.clone());
        Ok(module)
    }

    /// 出网白名单匹配（glob 近似：前缀 + `**` 尾通配）
    pub(crate) fn url_allowed_for(
        manifest: &AdapterManifest,
        url: &str,
    ) -> bool {
        Self::url_allowed(manifest, url)
    }

    /// 白名单判据（`adapter_host` 的 http_fetch 与 /ptgen 试探共用这一份）
    pub(crate) fn url_allowed(manifest: &AdapterManifest, url: &str) -> bool {
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
                // base/** 模式：域名段必须精确等于 base，或以 base/ 开头
                // （四审 L3：纯 starts_with(base) 放行 evil.tld 前缀仿冒域）
                host_path == base || host_path.starts_with(&format!("{base}/"))
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
        secrets: HashMap<String, String>,
    ) -> DomainResult<String> {
        let module = self.module_cached(manifest, wasm_bytes)?;
        let m = AdapterManifest {
            adapter_id: manifest.adapter_id.clone(),
            kind: manifest.kind.clone(),
            http_allow: manifest.http_allow.clone(),
            secrets_read: manifest.secrets_read.clone(),
            rate_limit_per_min: manifest.rate_limit_per_min,
        };
        let rate = self.bucket_for(&m.adapter_id);
        let mut store =
            new_store(&self.engine, m, secrets, http, self.kv.clone(), rate);
        // epoch bump 线程：EPOCH_TIMEOUT 后触发 trap
        let engine = self.engine.clone();
        let bump = std::thread::spawn(move || {
            std::thread::sleep(EPOCH_TIMEOUT);
            engine.increment_epoch();
        });

        let linker = build_linker(&self.engine)?;

        let instance =
            linker.instantiate(&mut store, &module).map_err(|e| {
                adapter_err(manifest, &format!("wasm 实例化失败: {e}"))
            })?;
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
            .call(
                &mut store,
                (in_region as i32, url_bytes.len() as i32, out_region as i32),
            )
            .map_err(|e| {
                adapter_err(manifest, &format!("wasm 执行失败: {e}"))
            })?;
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

// Component 模型 v2 预留（M4 用核心 wasm + 手写 ABI）
#[allow(dead_code)]
fn _component_placeholder(_: Component) {}
