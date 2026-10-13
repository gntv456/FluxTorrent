//! 适配器的**调用侧**（从 `adapter_http.rs` 按域拆出，批四十九）：
//! 行→清单、熔断唯一维护点 `adapter_call`、出网执行器。
//!
//! 拆这一刀的动因：`adapter_http.rs` 是 596 行的存量超限件（「只许瘦不许胖」），
//! 而 P-190（装上去的模块指纹没有任何读口）要往里加行 —— 先把调用域搬出去。

use actix_web::{web, HttpResponse};
use serde_json::Value;
use std::sync::Arc;

use crate::adapter_runtime::{AdapterManifest, AdapterRuntime};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

const STRIKE_LIMIT: i32 = 3;

fn internal<E: Into<anyhow::Error>>(e: E) -> DomainError {
    DomainError::Internal(e.into())
}

/// adapters 表行 → 运行时清单
pub(crate) fn row_to_manifest(
    adapter_id: String,
    kind: String,
    http_allow: Value,
    secrets_read: Value,
    rate_limit_per_min: i32,
) -> AdapterManifest {
    AdapterManifest {
        adapter_id,
        kind,
        http_allow: http_allow
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        secrets_read: secrets_read
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        rate_limit_per_min,
    }
}

// ============ 调用入口（熔断唯一维护点） ============

/// 适配器调用：Err → strikes+1（≥3 停用）；Ok → 清零。
/// 返回 (结果 JSON 字符串)。
pub(crate) async fn adapter_call(
    state: &web::Data<std::sync::Arc<AppState>>,
    adapter_id: &str,
    url: &str,
) -> DomainResult<HttpResponse> {
    let row: Option<(String, String, Value, Value, i32, bool, Vec<u8>)> =
        sqlx::query_as(
            "SELECT adapter_id, kind, http_allow, secrets_read, \
         rate_limit_per_min, enabled, wasm FROM adapters \
         WHERE adapter_id = $1",
        )
        .bind(adapter_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(internal)?;
    let Some((aid, kind, allow, secrets, rate, enabled, wasm)) = row else {
        return Err(DomainError::Validation("适配器不存在".into()));
    };
    if !enabled {
        return Err(DomainError::Validation(format!(
            "适配器 {aid} 未启用（或已被熔断停用）"
        )));
    }
    let manifest = row_to_manifest(aid, kind, allow, secrets, rate);
    // secret 预加载：仅 manifest.secrets_read 列出的键（调用前一次查库）
    let mut secret_map = std::collections::HashMap::new();
    if !manifest.secrets_read.is_empty() {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT name, value FROM site_settings \
             WHERE name = ANY($1)",
        )
        .bind(
            &manifest
                .secrets_read
                .iter()
                .map(|n| format!("adapter_secret_{n}"))
                .collect::<Vec<_>>(),
        )
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
        for (full, val) in rows {
            if let Some(short) = full.strip_prefix("adapter_secret_") {
                secret_map.insert(short.to_string(), val);
            }
        }
    }
    // 四轮审计 M1（2026-10-07）：出网执行器复用 adapter_http_exec（禁重定向
    // + 统一 UA/超时）——此前这里内联 Client::new() 默认跟随重定向，白名单
    // 域 302 → 内网即可绕过 http_allow（admin-try 路径 SSRF）。
    let http = adapter_http_exec();
    let wasm = wasm.clone();
    let manifest_err = manifest.adapter_id.clone();
    let result = {
        let rt = AdapterRuntime::global();
        let m = AdapterManifest {
            adapter_id: manifest.adapter_id.clone(),
            kind: manifest.kind.clone(),
            http_allow: manifest.http_allow.clone(),
            secrets_read: manifest.secrets_read.clone(),
            rate_limit_per_min: manifest.rate_limit_per_min,
        };
        let wasm2 = wasm.clone();
        let url = url.to_string();
        let secrets2 = secret_map.clone();
        tokio::task::spawn_blocking(move || {
            rt.call(&m, &wasm2, &url, http, secrets2)
        })
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
    };
    // guest 可能把失败折成 success:false JSON（SDK 口径）——视为 Err 计熔断
    let result = match result {
        Ok(out) => {
            let v: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
            if v.get("success").and_then(Value::as_bool) == Some(false) {
                Err(DomainError::Validation(format!(
                    "适配器 {}: {}",
                    manifest.adapter_id,
                    v.get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("执行失败"),
                )))
            } else {
                Ok(v)
            }
        }
        Err(e) => Err(e),
    };
    match result {
        Ok(out) => {
            sqlx::query(
                "UPDATE adapters SET strikes = 0, last_error = '' \
                 WHERE adapter_id = $1",
            )
            .bind(&manifest.adapter_id)
            .execute(&state.repo.db)
            .await
            .ok();
            Ok(ok(serde_json::json!({
                "adapter_id": manifest.adapter_id, "result": out,
            })))
        }
        Err(e) => {
            let msg = e.to_string();
            let strikes: i32 = sqlx::query_scalar(
                "UPDATE adapters SET strikes = strikes + 1, \
                 last_error = $2 WHERE adapter_id = $1 \
                 RETURNING strikes",
            )
            .bind(&manifest_err)
            .bind(&msg)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(1);
            if strikes >= STRIKE_LIMIT {
                sqlx::query(
                    "UPDATE adapters SET enabled = FALSE \
                     WHERE adapter_id = $1",
                )
                .bind(&manifest_err)
                .execute(&state.repo.db)
                .await
                .ok();
                tracing::error!(
                    adapter = manifest_err.as_str(),
                    strikes,
                    "适配器熔断停用（连续失败 ≥3）"
                );
            }
            Err(e)
        }
    }
}

/// 共用出网执行器（同步签名；spawn_blocking 线程内 Handle::block_on 桥）
pub(crate) fn adapter_http_exec(
) -> Arc<dyn Fn(&str) -> Result<String, String> + Send + Sync> {
    Arc::new(move |u: &str| -> Result<String, String> {
        let rt = tokio::runtime::Handle::current();
        let u = u.to_string();
        rt.block_on(async move {
            // P2（2026-10-06 安全审计）：禁跟随重定向——http_allow 白名单域
            // 302 跳内网地址（169.254.169.254 等）必须拒绝而不是跟随
            let client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| e.to_string())?;
            let resp = client
                .get(&u)
                .header(
                    "User-Agent",
                    concat!(
                        "Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 ",
                        "like Mac OS X) AppleWebKit/605.1.15 ",
                        "Mobile/15E148 Safari/604.1",
                    ),
                )
                .timeout(std::time::Duration::from_secs(
                    crate::http::OUTBOUND_FETCH_TIMEOUT_SECS,
                ))
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = resp.status().as_u16();
            let text = resp.text().await.map_err(|e| e.to_string())?;
            if !(200..300).contains(&status) {
                return Err(format!("HTTP {status}"));
            }
            Ok(text)
        })
    })
}
