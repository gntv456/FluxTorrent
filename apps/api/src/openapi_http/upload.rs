//! 开放 API：Token 化发种（0267）。
//!
//! 定位：PT-depiler / MoviePilot 这类「自动发种」工具的接入点 ——
//! 此前只有网页会话发种，Token 调不通，发种自动化整条链路不可用。
//! 与网页上传**共用同一条核心链路**（权限/过审/扣费/幂等完全一致），
//! 差别只在鉴权来源：会话 JWT → API Token 合成的调用上下文。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;
use crate::publish_http::{upload_token, UploadOutcome};
use crate::state::AppState;

use super::{no_store, require_scope, require_token, with_rl, SCOPE_UPLOAD};

/// 用 API Token 合成会话上下文。
/// class_id 直读 users —— Token 不固化角色，权限仍按用户**当前**等级实时判定，
/// 降级/封禁后旧 Token 立刻失去相应权限（与 require_token 的 JOIN 口径一致）。
async fn auth_from_token(
    state: &web::Data<std::sync::Arc<AppState>>,
    uid: i64,
) -> DomainResult<AuthUser> {
    let class_id: Option<i32> = sqlx::query_scalar(
        "SELECT class_id FROM users WHERE id = $1 AND status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(class_id) = class_id else {
        return Err(DomainError::Unauthorized);
    };
    Ok(AuthUser {
        id: uid,
        class_id,
        iat: chrono::Utc::now().timestamp(),
    })
}

/// POST /open/torrents —— Token 化发种。
///
/// 请求：`multipart/form-data`
///   - `file`（必填）：.torrent 字节，≤ 4MiB
///   - `nfo`（可选）：NFO 文本，≤ 1MiB
///   元数据走查询参数（与网页发种同一套 UploadForm 字段）：
///   `category_id`（必填）、`name`、`small_descr`、`descr`、`anonymous`、
///   `price`、`sections`（JSON 串）等。
///
/// 幂等：按 `info_hash` / `raw_info_hash` 判重，已存在时返回 200 且
/// `duplicate=true` 并附上既有 id —— 工具重试同一 .torrent 不会失败。
#[post("/open/torrents")]
pub(super) async fn open_upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    payload: actix_multipart::Multipart,
) -> DomainResult<HttpResponse> {
    let tk = require_token(&req, &state).await?;
    // 0267 安全闸：发种是写操作，必须显式持有 upload scope。
    // 此前 scopes 只落库不校验 —— 一枚声明 read-only 的 Token 也能发种。
    require_scope(&tk, SCOPE_UPLOAD)?;
    // 发种独立限流（每用户 20 次/分钟）：Token 默认 60 req/min 对「发种」
    // 这个动作太宽松（60 个种子/分钟足以灌爆审核队列）
    {
        use redis::AsyncCommands;
        let mut c = state.redis.clone();
        let key = format!("rl:openup:{}", tk.uid);
        let n: i64 = c.incr(&key, 1).await.unwrap_or(0);
        if n == 1 {
            let _: () = c.expire(&key, 60).await.unwrap_or(());
        }
        if n > 20 {
            return Err(DomainError::RateLimited);
        }
    }
    let auth = auth_from_token(&state, tk.uid).await?;
    // 复用网页上传的查询参数解析（不把 UploadForm 提升为对外契约）
    let qs = req.query_string().to_string();
    let out = upload_token(&state, &auth, &qs, payload).await?;
    let body = match out {
        UploadOutcome::Created(mut v) => {
            if let Some(o) = v.as_object_mut() {
                o.insert("duplicate".into(), serde_json::Value::Bool(false));
            }
            v
        }
        UploadOutcome::Duplicate {
            id,
            info_hash,
            pieces_hash,
        } => serde_json::json!({
            "id": id,
            "duplicate": true,
            "info_hash": info_hash,
            "pieces_hash": pieces_hash,
            "hint": "种子已存在（按 info_hash/raw_info_hash 幂等），未重复入库",
        }),
    };
    Ok(no_store(with_rl(ok(body), &tk)))
}
