//! 适配器的**宿主 ABI**（批四十九按域拆自 `adapter_runtime.rs`）：
//! guest 可调用的五个 host 导入——`http_fetch` / `cache_get` / `cache_set` /
//! `host_log` / `secret`。返回码约定（`adapter_runtime.rs` 的负码→原话表依赖）：
//! -1 内存/UTF-8 不可读，-2 出网越权，-3 限流，-4 上游失败，-5 secret 越权。
//!
//! 沙箱件（Store、内存上限、KV 表与限流桶本体）在 `adapter_sandbox.rs`。

use wasmtime::{Engine, Linker};

use crate::adapter_sandbox::{guest_mem, read_str, write_str, HostState};
use crate::errors::{DomainError, DomainResult};

/// 注册四个 host 导入。返回码约定（`adapter_runtime.rs` 的负码→原话表依赖它）：
/// -1 内存/UTF-8 不可读，-2 出网越权，-3 限流，-4 上游失败，-5 secret 越权。
pub(crate) fn build_linker(
    engine: &Engine,
) -> DomainResult<Linker<HostState>> {
    let mut linker = Linker::new(engine);
    linker
        .func_wrap(
            "host",
            "http_fetch",
            |mut caller: wasmtime::Caller<'_, HostState>,
             url_ptr: i32,
             url_len: i32,
             out_ptr: i32,
             out_cap: i32|
             -> i32 {
                let Some((data, store)) = guest_mem(&mut caller) else {
                    return -1;
                };
                let url = match read_str(data, url_ptr, url_len) {
                    Some(s) => s,
                    None => return -1,
                };
                if !crate::adapter_runtime::AdapterRuntime::url_allowed(
                    &store.manifest,
                    &url,
                ) {
                    tracing::warn!(
                        adapter = store.manifest.adapter_id.as_str(),
                        %url,
                        "出网越权拒绝"
                    );
                    return -2;
                }
                if !store.rate_take() {
                    tracing::warn!(
                        adapter = store.manifest.adapter_id.as_str(),
                        %url,
                        "适配器限流拒绝"
                    );
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
                let Some((data, store)) = guest_mem(&mut caller) else {
                    return -1;
                };
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
                let Some((data, store)) = guest_mem(&mut caller) else {
                    return;
                };
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
            "host_log",
            |mut caller: wasmtime::Caller<'_, HostState>,
             level: i32,
             msg_ptr: i32, msg_len: i32| {
                let Some((data, store)) = guest_mem(&mut caller) else {
                    return;
                };
                if let Some(m) = read_str(data, msg_ptr, msg_len) {
                    match level {
                        2 => tracing::warn!(
                            adapter = store.manifest.adapter_id.as_str(),
                            "{m}"
                        ),
                        _ => tracing::info!(
                            adapter = store.manifest.adapter_id.as_str(),
                            "{m}"
                        ),
                    }
                }
            },
        ))
        .and_then(|l| l.func_wrap(
            "host",
            "secret",
            |mut caller: wasmtime::Caller<'_, HostState>,
             name_ptr: i32, name_len: i32,
             out_ptr: i32, out_cap: i32|
             -> i32 {
                let Some((data, store)) = guest_mem(&mut caller) else {
                    return -1;
                };
                let name = match read_str(data, name_ptr, name_len) {
                    Some(s) => s,
                    None => return -1,
                };
                if !store.manifest.secrets_read.contains(&name) {
                    // 白名单外请求：不留内容痕迹，长度 -5 区分
                    return -5;
                }
                match store.secrets.get(&name) {
                    Some(v) => write_str(data, out_ptr, out_cap, v),
                    None => 0,
                }
            },
        ))
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(linker)
}
