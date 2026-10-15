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
pub(crate) fn build_linker(engine: &Engine) -> DomainResult<Linker<HostState>> {
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
        .and_then(|l| {
            l.func_wrap(
                "host",
                "cache_get",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 key_ptr: i32,
                 key_len: i32,
                 out_ptr: i32,
                 out_cap: i32|
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
            )
        })
        .and_then(|l| {
            l.func_wrap(
                "host",
                "cache_set",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 key_ptr: i32,
                 key_len: i32,
                 val_ptr: i32,
                 val_len: i32,
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
            )
        })
        .and_then(|l| {
            l.func_wrap(
                "host",
                "host_log",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 level: i32,
                 msg_ptr: i32,
                 msg_len: i32| {
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
            )
        })
        .and_then(|l| {
            l.func_wrap(
                "host",
                "secret",
                |mut caller: wasmtime::Caller<'_, HostState>,
                 name_ptr: i32,
                 name_len: i32,
                 out_ptr: i32,
                 out_cap: i32|
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
            )
        })
        .map_err(|e| DomainError::Internal(e.into()))?;
    // LLM 宿主能力（2026-10-15）：按 manifest.llm_routes 白名单，默认空 = 不给能力。一与前五个导入同一套返回码约定。
    register_llm(&mut linker)?;
    Ok(linker)
}

// ============ LLM 宿主能力（2026-10-15） ============
//
// 对标 PT Studio 的 llm-claude / ai-assistant 一族。**只做宿主能力，不做
// 平台**：不引 agent 框架、不做多轮会话管理、不存对话历史——那些是
// 产品决策，不是补缺口。要点见 `_doc/PT-Studio终局能力与FluxTorrent借鉴
// 评估.md` §LLM/MCP（「可选战略方向，不是 PT 刚需」）。
//
// 为什么放在 wasmtime 适配器层而不是照搬 ctx.llm/MCP 内核：
//   * 适配器层已有**最严**的安全边界（沙箱 + 出网白名单 + 限流 + 能力
//     声明）。新能力挂这里，天然继承那套边界；挂到核心业务里做，就是
//     又开一个能直连 PG 的口子。
//   * 契约层（P1 `extension.rs`）的 `Capabilities` 加 `llm_routes` 字段，
//     声明式闸门与既有 `net_egress`/`secret_read` 同款。
//
// 失败姿态：**默认关闭**。未配 `LLM_BASE_URL`/`LLM_API_KEY` 时所有调用
// 返回 -4（上游失败），不留半可用状态——一个「配了 key 才能用」的能力
// 比「明确不可用」更难排查。
//
// 返回码沿用既有负码表（`adapter_runtime.rs` 依赖它做原话映射）：
// -1 内存/UTF-8 不可读，-2 能力越权（未声明 llm:route），-3 限流，
// -4 上游未配置/失败，-5 保留给 secret。

/// LLM 路由是否已在本站配置（启动期一次性读，避免每次调用查 env）。
pub(crate) fn llm_configured() -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        std::env::var("LLM_BASE_URL").is_ok()
            && std::env::var("LLM_API_KEY").is_ok()
    })
}

/// 调用 OpenAI 兼容的 chat/completions。非流式、只取 content。
///
// 用 reqwest 的 blocking 客户端（仓内已有该依赖，rustls-tls，不引新包）：
// 适配器 hook 跑在 spawn_blocking 的同步上下文里，async 客户端在这里没法用
// （没有 runtime handle 驱动的 await 点）。
fn llm_chat(route: &str, prompt: &str, max_tokens: i32) -> Result<String, i32> {
    if !llm_configured() {
        return Err(-4);
    }
    let base = std::env::var("LLM_BASE_URL").map_err(|_| -4)?;
    let key = std::env::var("LLM_API_KEY").map_err(|_| -4)?;
    // route 形如 "anthropic/claude-sonnet" 或 "openai/gpt-4o-mini"；
    // 只允许出现在声明的白名单里，所以这里不再校验字符集。
    let url = format!("{}/chat/completions", base.trim_end_matches('/'));
    let body = serde_json::json!({
        "model": route,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
        "stream": false,
    })
    .to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|_| -4)?;
    let text = client
        .post(&url)
        .bearer_auth(&key)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .map_err(|_| -4)?
        .text()
        .map_err(|_| -4)?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| -4)?;
    v["choices"][0]["message"]["content"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or(-4)
}

/// 注册 LLM 宿主导入：`host.llm_chat(route_ptr, route_len, prompt_ptr,
/// prompt_len, out_ptr, out_cap) -> i32`。
///
/// 闸门顺序刻意与既有导入一致：**先查能力，再查限流，最后才出网**——
/// 越权调用不该消耗配额。
pub(crate) fn register_llm(
    linker: &mut Linker<HostState>,
) -> DomainResult<()> {
    linker
        .func_wrap(
            "host",
            "llm_chat",
            |mut caller: wasmtime::Caller<'_, HostState>,
             route_ptr: i32,
             route_len: i32,
             prompt_ptr: i32,
             prompt_len: i32,
             out_ptr: i32,
             out_cap: i32|
             -> i32 {
                let Some((data, store)) = guest_mem(&mut caller) else {
                    return -1;
                };
                let route = match read_str(data, route_ptr, route_len) {
                    Some(s) => s,
                    None => return -1,
                };
                let prompt = match read_str(data, prompt_ptr, prompt_len) {
                    Some(s) => s,
                    None => return -1,
                };
                // 能力闸门：路由必须在 manifest 声明的白名单里。
                if !store.manifest.llm_routes.iter().any(|r| r == &route) {
                    tracing::warn!(
                        adapter = store.manifest.adapter_id.as_str(),
                        %route,
                        "LLM 路由越权拒绝"
                    );
                    return -2;
                }
                if !store.rate_take() {
                    tracing::warn!(
                        adapter = store.manifest.adapter_id.as_str(),
                        %route,
                        "适配器限流拒绝（LLM）"
                    );
                    return -3;
                }
                match llm_chat(&route, &prompt, 1024) {
                    Ok(s) => write_str(data, out_ptr, out_cap, &s),
                    Err(code) => code,
                }
            },
        )
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

#[cfg(test)]
mod llm_tests {
    use super::llm_configured;

    #[test]
    fn llm_is_off_without_env() {
        // 本仓 CI 不配 LLM_* —— 这个断言必须成立，否则测试会在有 key 的
        // 开发机上反向失败（那不是产品红，是环境差异）。
        if std::env::var("LLM_BASE_URL").is_err() {
            assert!(!llm_configured());
        }
    }
}
