//! 封禁公示列表（GET /ban-log）——样图 § 信息与辅助 / 封禁记录。
//!
//! SQL 住在 repo/community/ban_log.rs（H 层零容忍带 SQL）；本文件只做
//! 鉴权、参数取值与信封组装。
//!
//! 三条口径：
//! * **登录可读、不需要权限键**——这是公示面不是职员面（与 GET /users 同档）。
//!   现网 admin_http/user_status.rs:177 那句注释早就写着「用户只能从公开
//!   ban-log 猜状态」，可此前只有按用户名点查的 /auth/ban-log，没有名单。
//! * 用户名在仓储层就已经打码，这里拿不到原名。
//! * `kind` 只认 ban|mute，其余值按「不筛」处理——公示页宁可多列几行，
//!   也不要因为一个拼错的查询参数就演成「本站没有封禁记录」。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::repo::community::ban_log as repo_ban;
use crate::state::AppState;

const PER_MAX: i64 = 50;
const PER_DEFAULT: i64 = 20;

#[derive(Deserialize)]
struct BanLogQuery {
    /// 用户名模糊检索（绑定进 SQL）
    #[serde(default)]
    q: Option<String>,
    /// ban | mute | 空=全部
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    page: Option<i64>,
    #[serde(default)]
    per: Option<i64>,
}

#[get("/ban-log")]
pub(crate) async fn ban_log_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<BanLogQuery>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let page = q.page.unwrap_or(1).clamp(1, 10_000);
    let per = q.per.unwrap_or(PER_DEFAULT).clamp(1, PER_MAX);
    let needle = q
        .q
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(64).collect::<String>());
    let kind: Option<i16> = match q.kind.as_deref() {
        Some("ban") => Some(2),
        Some("mute") => Some(1),
        _ => None,
    };
    let rows = repo_ban::ban_log_page(
        &state.repo,
        needle.as_deref(),
        kind,
        per,
        (page - 1) * per,
    )
    .await?;
    let total =
        repo_ban::ban_log_count(&state.repo, needle.as_deref(), kind).await?;
    Ok(ok(serde_json::json!({
        "rows": rows, "total": total, "page": page, "per": per,
        "kind": q.kind.as_deref().unwrap_or(""),
    })))
}
