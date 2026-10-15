//! 适配器 HTTP 层（生态商店 M4，策划案 §5.4）：安装/启停/列表/试调 + PT-Gen 接线。
//!
//! 熔断（A4）：`adapter_call` 是唯一调用入口——Err 时 strikes+1（≥3 自动
//! enabled=false + last_error 留痕），Ok 时清零。/ptgen 路径「适配器优先、
//! PT-Gen 回退」：适配器 enabled 且 host 命中其 http_allow 前缀才走沙箱；
//! 失败自动回退 PT-Gen（站长侧无感，降级口径见策划案 §5.1）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::Value;

use crate::adapter_call::{adapter_call, adapter_http_exec, row_to_manifest};
use crate::adapter_runtime::{AdapterManifest, AdapterRuntime};
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

const MAX_WASM_BYTES: usize = 2 * 1024 * 1024;

fn internal<E: Into<anyhow::Error>>(e: E) -> DomainError {
    DomainError::Internal(e.into())
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
        return Err(DomainError::Validation("wasm 需为 1B–2MiB".into()));
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
    state
        .repo
        .audit(Some(auth.id), "adapter:install", None)
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
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
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
        i64,
        String,
        String,
        String,
        String,
        Value,
        i32,
        bool,
        i32,
        String,
        chrono::DateTime<chrono::Utc>,
        String,
    )> = sqlx::query_as(
        "SELECT id, adapter_id, kind, name, version, http_allow, \
         rate_limit_per_min, enabled, strikes, last_error, installed_at, \
         checksum FROM adapters ORDER BY kind, adapter_id",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(ok(rows
        .into_iter()
        .map(
            |(
                id,
                adapter_id,
                kind,
                name,
                version,
                allow,
                rate,
                enabled,
                strikes,
                last_error,
                at,
                checksum,
            )| {
                serde_json::json!({
                    "id": id, "adapter_id": adapter_id, "kind": kind,
                    "name": name, "version": version, "http_allow": allow,
                    "rate_limit_per_min": rate, "enabled": enabled,
                    "strikes": strikes, "last_error": last_error,
                    "installed_at": at, "checksum": checksum,
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
            if body.enabled {
                "adapter:enable"
            } else {
                "adapter:disable"
            },
            None,
        )
        .await;
    Ok(ok(serde_json::json!({
        "adapter_id": body.adapter_id, "enabled": body.enabled,
    })))
}

#[derive(Deserialize)]
struct AdapterDeleteBody {
    adapter_id: String,
}

/// 卸载适配器（闭环审查 B10：此前只有安装/启停，装了坏插件无法撤——只能改库）。
/// 与 install 对称：需 SETTINGS_MANAGE，审计留痕；连带清掉 wasm 字节（表行删除）。
#[post("/admin/adapters/delete")]
async fn adapter_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<AdapterDeleteBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let adapter_id = body.adapter_id.trim();
    if adapter_id.is_empty() {
        return Err(DomainError::Validation("adapter_id 必填".into()));
    }
    let n = sqlx::query("DELETE FROM adapters WHERE adapter_id = $1")
        .bind(adapter_id)
        .execute(&state.repo.db)
        .await
        .map_err(internal)?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::Validation("适配器不存在".into()));
    }
    state
        .repo
        .audit(Some(auth.id), "adapter:delete", None)
        .await;
    Ok(ok(serde_json::json!({ "deleted": adapter_id })))
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

/// PT-Gen 路径的适配器优先钩子：启用的 metadata 适配器且 URL 命中其
/// http_allow → 沙箱执行；guest JSON（{success,name,poster,descr}）折成
/// PT-Gen 同形出口。任何失败 → None（调用方回退 PT-Gen，降级语义 §5.1）。
/// 注意：此路径不维护 strikes（只读试探；熔断计数在 /admin/adapters/try
/// 与未来的常规调用入口维护）。
pub(crate) async fn try_adapter_metadata(
    state: &web::Data<std::sync::Arc<AppState>>,
    url: &str,
) -> Option<(String, String)> {
    let rows: Vec<(String, Value, Value, i32, Vec<u8>)> = sqlx::query_as(
        "SELECT adapter_id, http_allow, secrets_read,          rate_limit_per_min, wasm FROM adapters          WHERE enabled AND kind = 'metadata'",
    )
    .fetch_all(&state.repo.db)
    .await
    .ok()?;
    for (aid, allow, secrets, rate, wasm) in rows {
        let manifest =
            row_to_manifest(aid, String::new(), allow, secrets, rate);
        if !AdapterRuntime::url_allowed_for(&manifest, url) {
            continue;
        }
        // secret 预加载（白名单内）
        let mut secret_map = std::collections::HashMap::new();
        if !manifest.secrets_read.is_empty() {
            let sk: Vec<(String, String)> = sqlx::query_as(
                "SELECT name, value FROM site_settings                  WHERE name = ANY($1)",
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
            for (full, val) in sk {
                if let Some(short) = full.strip_prefix("adapter_secret_") {
                    secret_map.insert(short.to_string(), val);
                }
            }
        }
        let http = adapter_http_exec();
        let rt = AdapterRuntime::global();
        let m2 = AdapterManifest {
            adapter_id: manifest.adapter_id.clone(),
            kind: manifest.kind.clone(),
            http_allow: manifest.http_allow.clone(),
            secrets_read: manifest.secrets_read.clone(),
            rate_limit_per_min: manifest.rate_limit_per_min,
            llm_routes: manifest.llm_routes.clone(),
        };
        let url2 = url.to_string();
        let out = tokio::task::spawn_blocking(move || {
            rt.call(&m2, &wasm, &url2, http, secret_map)
        })
        .await
        .ok()?
        .ok()?;
        let v: Value = serde_json::from_str(&out).ok()?;
        if v.get("success").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let name = v
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut descr = v
            .get("descr")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if let Some(poster) = v
            .get("poster")
            .and_then(Value::as_str)
            .filter(|p| !p.is_empty())
        {
            descr = format!(
                "[img]{poster}[/img]
{descr}"
            );
        }
        return Some((name, descr));
    }
    None
}

pub fn mount_adapters(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(adapter_install)
        .service(adapter_list)
        .service(adapter_toggle)
        .service(adapter_delete)
        .service(adapter_try)
}
