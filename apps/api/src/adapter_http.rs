//! 适配器 HTTP 层（生态商店 M4，策划案 §5.4）：安装/启停/列表/试调 + PT-Gen 接线。
//!
//! 熔断（A4）：`adapter_call` 是唯一调用入口——Err 时 strikes+1（≥3 自动
//! enabled=false + last_error 留痕），Ok 时清零。/ptgen 路径「适配器优先、
//! PT-Gen 回退」：适配器 enabled 且 host 命中其 http_allow 前缀才走沙箱；
//! 失败自动回退 PT-Gen（站长侧无感，降级口径见策划案 §5.1）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

use crate::adapter_runtime::{AdapterManifest, AdapterRuntime};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

const MAX_WASM_BYTES: usize = 2 * 1024 * 1024;
const STRIKE_LIMIT: i32 = 3;

fn internal<E: Into<anyhow::Error>>(e: E) -> DomainError {
    DomainError::Internal(e.into())
}

/// adapters 表行 → 运行时清单
fn row_to_manifest(
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

// ============ 管理端点（sysop） ============

#[derive(Deserialize)]
struct AdapterInstallBody {
    adapter_id: String,
    kind: String,
    name: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    http_allow: Vec<String>,
    #[serde(default)]
    secrets_read: Vec<String>,
    #[serde(default)]
    rate_limit_per_min: Option<i32>,
    /// base64(wasm 模块字节)
    wasm_b64: String,
}

#[post("/admin/adapters/install")]
async fn adapter_install(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdapterInstallBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    if !["metadata", "payment", "indexer"].contains(&body.kind.as_str()) {
        return Err(DomainError::Validation(
            "kind 需为 metadata / payment / indexer".into(),
        ));
    }
    let adapter_id = body.adapter_id.trim();
    if adapter_id.is_empty() || adapter_id.len() > 100 {
        return Err(DomainError::Validation(
            "adapter_id 必填且 ≤100 字符".into(),
        ));
    }
    use base64::Engine as _;
    let wasm = base64::engine::general_purpose::STANDARD
        .decode(body.wasm_b64.trim())
        .map_err(|_| DomainError::Validation("wasm_b64 解码失败".into()))?;
    if wasm.is_empty() || wasm.len() > MAX_WASM_BYTES {
        return Err(DomainError::Validation(
            "wasm 需为 1B–2MiB".into(),
        ));
    }
    // 编译预检：坏模块直接拒绝（不能装一个起不来的适配器）
    wasmtime::Module::new(&AdapterRuntime::global().engine, &wasm)
        .map_err(|e| DomainError::Validation(format!("wasm 校验失败：{e}")))?;
    let checksum = sha256_hex(&wasm);
    let rate = body.rate_limit_per_min.unwrap_or(30).clamp(1, 600);
    let version = body.version.clone().unwrap_or_else(|| "1.0.0".into());
    sqlx::query(
        "INSERT INTO adapters (adapter_id, kind, name, version, \
         http_allow, secrets_read, rate_limit_per_min, wasm, checksum, \
         installed_by) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
         ON CONFLICT (adapter_id) DO UPDATE SET kind = EXCLUDED.kind, \
         name = EXCLUDED.name, version = EXCLUDED.version, \
         http_allow = EXCLUDED.http_allow, \
         secrets_read = EXCLUDED.secrets_read, \
         rate_limit_per_min = EXCLUDED.rate_limit_per_min, \
         wasm = EXCLUDED.wasm, checksum = EXCLUDED.checksum, \
         installed_at = now(), installed_by = EXCLUDED.installed_by, \
         enabled = FALSE, strikes = 0, last_error = ''",
    )
    .bind(adapter_id)
    .bind(&body.kind)
    .bind(&body.name)
    .bind(&version)
    .bind(serde_json::json!(body.http_allow))
    .bind(serde_json::json!(body.secrets_read))
    .bind(rate)
    .bind(&wasm)
    .bind(&checksum)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(internal)?;
    state.repo.audit(
        Some(auth.id),
        "adapter:install",
        None,
    )
    .await;
    Ok(ok(serde_json::json!({
        "installed": adapter_id, "version": version, "checksum": checksum,
        "enabled": false,
    })))
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[get("/admin/adapters")]
async fn adapter_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let rows: Vec<(
        i64, String, String, String, String, Value, i32, bool, i32,
        String, chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT id, adapter_id, kind, name, version, http_allow, \
         rate_limit_per_min, enabled, strikes, last_error, installed_at \
         FROM adapters ORDER BY kind, adapter_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows
        .into_iter()
        .map(
            |(id, adapter_id, kind, name, version, allow, rate, enabled, strikes, last_error, at)| {
                serde_json::json!({
                    "id": id, "adapter_id": adapter_id, "kind": kind,
                    "name": name, "version": version, "http_allow": allow,
                    "rate_limit_per_min": rate, "enabled": enabled,
                    "strikes": strikes, "last_error": last_error,
                    "installed_at": at,
                })
            },
        )
        .collect::<Vec<_>>()))
}

#[derive(Deserialize)]
struct AdapterToggleBody {
    adapter_id: String,
    enabled: bool,
}

#[post("/admin/adapters/toggle")]
async fn adapter_toggle(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdapterToggleBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let n = sqlx::query(
        "UPDATE adapters SET enabled = $2, strikes = 0, last_error = '' \
         WHERE adapter_id = $1",
    )
    .bind(body.adapter_id.trim())
    .bind(body.enabled)
    .execute(&state.repo.db)
    .await
    .map_err(internal)?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("适配器不存在".into()));
    }
    state
        .repo
        .audit(
            Some(auth.id),
            if body.enabled { "adapter:enable" } else { "adapter:disable" },
            None,
        )
        .await;
    Ok(ok(serde_json::json!({
        "adapter_id": body.adapter_id, "enabled": body.enabled,
    })))
}

#[derive(Deserialize)]
struct AdapterTryBody {
    adapter_id: String,
    url: String,
}

#[post("/admin/adapters/try")]
async fn adapter_try(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdapterTryBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    adapter_call(&state, body.adapter_id.trim(), &body.url).await
}

// ============ 调用入口（熔断唯一维护点） ============

/// 适配器调用：Err → strikes+1（≥3 停用）；Ok → 清零。
/// 返回 (结果 JSON 字符串)。
async fn adapter_call(
    state: &web::Data<std::sync::Arc<AppState>>,
    adapter_id: &str,
    url: &str,
) -> DomainResult<HttpResponse> {
    let row: Option<(
        String,
        String,
        Value,
        Value,
        i32,
        bool,
        Vec<u8>,
    )> = sqlx::query_as(
        "SELECT adapter_id, kind, http_allow, secrets_read, \
         rate_limit_per_min, enabled, wasm FROM adapters \
         WHERE adapter_id = $1",
    )
    .bind(adapter_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    let Some((aid, kind, allow, secrets, rate, enabled, wasm)) = row
    else {
        return Err(DomainError::Validation("适配器不存在".into()));
    };
    if !enabled {
        return Err(DomainError::Validation(format!(
            "适配器 {aid} 未启用（或已被熔断停用）"
        )));
    }
    let manifest = row_to_manifest(aid, kind, allow, secrets, rate);
    // 出网执行器：同步签名（guest ABI），在 spawn_blocking 线程内借
    // tokio handle block_on 跑 async reqwest（reqwest 无 blocking feature）
    let http = Arc::new(
        move |u: &str| -> Result<String, String> {
            let rt = tokio::runtime::Handle::current();
            let u = u.to_string();
            rt.block_on(async move {
                let client = reqwest::Client::new();
                let resp = client
                    .get(&u)
                    .header("User-Agent", "FluxTorrent-Adapter/1.0")
                    .timeout(std::time::Duration::from_secs(20))
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
        },
    );
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
        tokio::task::spawn_blocking(move || rt.call(&m, &wasm2, &url, http))
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
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
                "adapter_id": manifest.adapter_id, "result": serde_json::from_str::<Value>(&out).unwrap_or(Value::String(out)),
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

pub fn mount_adapters(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(adapter_install)
        .service(adapter_list)
        .service(adapter_toggle)
        .service(adapter_try)
}
