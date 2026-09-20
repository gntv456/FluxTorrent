//! 生态兼容层 ③a：PT-Plugin-Plus 聚合端点（朱雀口径）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::openapi_http::require_token;
use crate::state::AppState;

/// `/api/plugins/ptppUserInfo`：按 PT-Plugin-Plus 的字段命名一次性返回用户面板数据。
/// 这是朱雀（zhuque.in）验证过的「自研站进插件生态的最短路径」——插件的用户信息卡片、
/// 等级/魔力展示、未读提醒全部取自本端点，字段名即插件 schema 的约定。
/// 鉴权走开放 API Token（比朱雀的 cookie+CSRF 更适合纯前端插件外的工具）。
#[get("/plugins/ptppUserInfo")]
async fn ptpp_user_info(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let (uid, _rpm) = require_token(&req, &state).await?;
    let row: Option<(
        i64,
        String,
        i64,
        i64,
        i64,
        i64,
        i32,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        "SELECT u.id, u.username, u.spark_balance, u.uploaded, u.downloaded, \
                u.seeding_size, \
                u.class_id, u.created_at \
         FROM users u WHERE u.id = $1 AND u.status < 2",
    )
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((
        id,
        name,
        bonus,
        uploaded,
        downloaded,
        seeding_size,
        class_id,
        join_time,
    )) = row
    else {
        return Err(DomainError::Unauthorized);
    };
    let class_name: String =
        sqlx::query_scalar("SELECT name FROM user_classes WHERE id = $1")
            .bind(class_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .unwrap_or_else(|| "LV0".into());
    let (seeding, leeching, invites, unread_messages): (i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT \
            (SELECT count(*) FROM snatches WHERE user_id = $1 AND seeding), \
            (SELECT count(*) FROM snatches WHERE user_id = $1 AND leeching), \
            (SELECT count(*) FROM invites WHERE inviter_id = $1 AND status = 0 \
                AND expires_at > now()), \
            (SELECT count(*) FROM messages WHERE receiver_id = $1 \
                AND read_at IS NULL)",
        )
        .bind(uid)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({
        // 插件契约字段（PT-Plugin-Plus TNode schema 口径）
        "id": id,
        "name": name,
        "bonus": bonus,
        "uploaded": uploaded,
        "downloaded": downloaded,
        "seeding": seeding,
        "leeching": leeching,
        "seedingSize": seeding_size,
        "invites": invites,
        "levelName": class_name,
        "joinTime": join_time,
        "messageCount": unread_messages,
        "isLogged": true,
    })))
}
