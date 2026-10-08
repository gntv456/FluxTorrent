//! 检查更新（0276）：拉 GitHub 最新 Release 与本地版本比对。
//! 独立成模块而非进 ops.rs——后者已近 300 行门禁，且「出网查更新」与
//! 「本机运维三件」本属不同域。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// Release 元数据的最小裁剪（只取版本页要展示的字段）
#[derive(serde::Deserialize)]
struct GhRelease {
    tag_name: String,
    name: Option<String>,
    published_at: Option<String>,
    html_url: Option<String>,
    body: Option<String>,
}

/// semver 三元组解析（"v0.3.1" → (0,3,1)；解析失败按 0 处理，只会显示「有更新」）
fn ver_tuple(s: &str) -> (u64, u64, u64) {
    let mut it = s.trim_start_matches('v').split('.').map(|p| {
        let digits: String = p.chars().filter(|c| c.is_ascii_digit()).collect();
        digits.parse::<u64>().unwrap_or(0)
    });
    (
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
        it.next().unwrap_or(0),
    )
}

/// 检查更新：最新 Release vs 本地版本（CARGO_PKG_VERSION，与镜像 tag 同源）。
///
/// 出网方向唯一依赖 github.com；失败（离线/被墙/GitHub 抖动）返回 503 文案
/// 而非 500——这是外部依赖不可达，不是站内故障。
#[get("/admin/update_check")]
async fn admin_update_check(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(&state, &auth, crate::authz::perm::STAFF_PANEL)
        .await?;
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(
            crate::http::OUTBOUND_FETCH_TIMEOUT_SECS,
        ))
        .build()
        .map_err(|e| DomainError::Internal(e.into()))?;
    let resp = client
        .get("https://api.github.com/repos/gntv456/FluxTorrent/releases/latest")
        .header("User-Agent", "FluxTorrent-update-check")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| {
            DomainError::Validation(format!("无法访问 GitHub：{e}"))
        })?;
    if !resp.status().is_success() {
        return Err(DomainError::Validation(format!(
            "GitHub 返回 {}（仓库无 Release 或网络受限）",
            resp.status()
        )));
    }
    let rel: GhRelease = resp.json().await.map_err(|e| {
        DomainError::Validation(format!("解析 Release 失败：{e}"))
    })?;
    let current = env!("CARGO_PKG_VERSION");
    Ok(ok(serde_json::json!({
        "current": current,
        "latest": rel.tag_name.trim_start_matches('v'),
        "up_to_date": ver_tuple(&rel.tag_name) <= ver_tuple(current),
        "release_name": rel.name,
        "published_at": rel.published_at,
        "url": rel.html_url,
        // Release notes 截断到 600 字符：版本卡内联展示，全文走 url
        "notes": rel.body.as_deref()
            .map(|b| b.chars().take(600).collect::<String>()),
    })))
}
