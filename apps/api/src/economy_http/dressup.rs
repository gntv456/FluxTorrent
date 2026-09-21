//! M25 装扮中心：列表/佩戴。
//! 从 economy_http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ M25 装扮中心 ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct DressupRow {
    item_id: i64,
    name: String,
    kind: String,
    price: i64,
    slot: Option<String>,
    owned: bool,
    wearing: bool,
}

/// 装扮列表（拥有状态 + 佩戴中）
#[get("/dressup/list")]
async fn dressup_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let rows: Vec<DressupRow> = sqlx::query_as(
                "SELECT si.id AS item_id, si.name, si.kind, si.price, \
         si.config->>'slot' AS slot, \
         EXISTS(SELECT 1 FROM user_dressups ud WHERE ud.user_id = $1 AND ud.item_id = si.id) AS owned, \
         COALESCE((SELECT ud.wearing FROM user_dressups ud WHERE ud.user_id = $1 AND ud.item_id = si.id), FALSE) AS wearing FROM shop_items si WHERE si.active AND si.kind IN ('avatar_frame','animated_avatar','rainbow_id','rainbow_name') ORDER BY si.price",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct WearReq {
    item_id: i64,
    wear: bool,
}

/// 佩戴/摘下（同类互斥由 DB 触发器保证：佩戴前先摘同类）
#[post("/dressup/wear")]
async fn dressup_wear(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<WearReq>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 槽位 + 装扮类型一起查：佩戴时按 kind 落到真实展示列（头像框→avatar_frame_id，
    // 动态头像→avatar_url 覆盖），否则装扮只在 user_dressups 里改标记、页面看不到效果
    let owned: Option<(Option<String>, String, Option<String>)> = sqlx::query_as(
                "SELECT si.config->>'slot', si.kind, \
         si.config->>'avatar_url' FROM user_dressups ud JOIN shop_items si ON si.id = ud.item_id WHERE ud.user_id = $1 AND ud.item_id = $2",
    )
    .bind(auth.id)
    .bind(body.item_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((slot, kind, effect_url)) = owned else {
        return Err(DomainError::Validation(
            "尚未拥有该装扮（先在商店购买）".into(),
        ));
    };

    if body.wear {
        // 先摘下同槽位（触发器互斥的最简前置）。注意 UPDATE...FROM 里目标表列不能带别名前缀
        sqlx::query(
            r#"UPDATE user_dressups ud SET wearing = FALSE FROM shop_items si
             WHERE si.id = ud.item_id AND ud.user_id = $1 AND si.config->>'slot' = $2"#,
        )
        .bind(auth.id)
        .bind(&slot)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query(
            "UPDATE user_dressups SET wearing = TRUE WHERE \
         user_id = $1 AND item_id = $2",
        )
        .bind(auth.id)
        .bind(body.item_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // 展示列落地（同槽位互斥）：avatar 槽二选一——
        //  avatar_frame     → users.avatar_frame_id（四季框/光环同链路，/me 与各页现查现回）
        //  animated_avatar  → users.avatar_url 覆盖为 config.avatar_url（gif 等动态图）
        match (kind.as_str(), slot.as_deref()) {
            ("avatar_frame", Some("avatar")) => {
                // config.avatar_frame_id 缺省时回落用价格最贵的一帧（管理员发放道具时可不填）
                let fid: Option<i32> =
                    match effect_url.as_deref().and_then(|s| s.parse().ok()) {
                        Some(x) => Some(x),
                        None => sqlx::query_scalar(
                            "SELECT id FROM avatar_frames \
                         ORDER BY price DESC, id DESC LIMIT 1",
                        )
                        .fetch_optional(&state.repo.db)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?
                        .flatten(),
                    };
                sqlx::query(
                    "UPDATE users SET avatar_frame_id = $2 WHERE id = $1",
                )
                .bind(auth.id)
                .bind(fid)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
            ("animated_avatar", Some("avatar")) => {
                let Some(url) = effect_url.as_deref().filter(|s| !s.is_empty())
                else {
                    return Err(DomainError::Validation(
                        "该动态头像未配置图片（config.avatar_url），请联系管理员".into(),
                    ));
                };
                sqlx::query("UPDATE users SET avatar_url = $2 WHERE id = $1")
                    .bind(auth.id)
                    .bind(url)
                    .execute(&state.repo.db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            }
            _ => {} // 用户名槽（rainbow_*）走样式列，暂无展示列需要落
        }
    } else {
        sqlx::query(
            "UPDATE user_dressups SET wearing = FALSE WHERE \
         user_id = $1 AND item_id = $2",
        )
        .bind(auth.id)
        .bind(body.item_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        // 摘下恢复：动态头像恢复空值（回退首字母/原头像由用户自查），头像框清佩戴
        if kind == "animated_avatar" && slot.as_deref() == Some("avatar") {
            sqlx::query("UPDATE users SET avatar_url = NULL WHERE id = $1")
                .bind(auth.id)
                .execute(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        } else if kind == "avatar_frame" && slot.as_deref() == Some("avatar") {
            sqlx::query(
                "UPDATE users SET avatar_frame_id = NULL WHERE id = $1",
            )
            .bind(auth.id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    state
        .repo
        .audit(
            Some(auth.id),
            if body.wear {
                "dressup.wear"
            } else {
                "dressup.takeoff"
            },
            Some(body.item_id),
        )
        .await;
    Ok(ok(
        serde_json::json!({ "item_id": body.item_id, "wearing": body.wear }),
    ))
}
