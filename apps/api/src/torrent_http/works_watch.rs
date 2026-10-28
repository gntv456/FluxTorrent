//! 作品订阅新作（0424 P1-2 从 works.rs 拆出，300 行门禁）。
//! 复用 wishlist 全链（20 条上限、24h 节流、3h 命中窗、聚合通知），
//! 不新建订阅实体。

use actix_web::{post, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 订阅作品新作（0424 P1-2）：把作品名写进愿望单——复用 wishlist 全链
/// （20 条上限、24h 节流、3h 命中窗、站内信+push 聚合通知），不新建订阅实体。
///
/// 关键词取作品的**归一化标题**而不是某一颗种的全名：本站种名普遍是
/// 「片名｜4K HDR｜类型…」，作品内各版本只在 ｜ 之后不同；用全名做
/// ILIKE 关键词时，同作品的 1080P 版/新集数永远不命中（订阅形同虚设）。
/// 归一化表达式与作品聚合共用 `works_sql::title_expr`，两侧不各写一份。
#[utoipa::path(
    post,
    path = "/api/v1/works/{key}/watch",
    tag = "种子",
    description = "订阅作品新作（复用愿望单通知链；上限 20 条）。",
    params(
        ("key" = String, description = "作品键 g12 / i tt… / m… / d… / n<md5>"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "作品不存在或无可见种子（code 1004）"),
        (status = 422, description = "参数校验失败（code 1002）"),
    )
)]
#[post("/works/{key}/watch")]
pub(crate) async fn work_watch(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    // 订阅写的是愿望单：wishlist 模块关闭时管理页/通知链都不可达，
    // 这里必须与 /wishlist 端点同口径（模块先于业务校验，4101）。
    state.require_module(crate::modules::key::WISHLIST).await?;
    let key = path.into_inner();
    // 键形态与详情口同一白名单（valid_work_key）：此前只做了前缀裁剪，
    // n-key 永远匹配不到（形同 404），m/d/i 数字键还会与 group_id 撞车。
    if !super::works::valid_work_key(&key) {
        return Err(DomainError::NotFound(0));
    }
    let name: Option<String> = sqlx::query_scalar(&format!(
        "SELECT COALESCE(NULLIF({title}, ''), t.name) \
         FROM torrents t \
         WHERE t.approval_status = 1 AND {wk} = $1 \
         ORDER BY (t.media_info->>'poster') IS NOT NULL DESC, \
                  t.created_at DESC, t.id DESC LIMIT 1",
        title = super::works_sql::title_expr(),
        wk = super::works_sql::wk_expr(),
    ))
    .bind(&key)
    .fetch_optional(&state.repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(name) = name else {
        return Err(DomainError::NotFound(0));
    };
    // 愿望单口径：长度按字数 1-100（正常片名远低于此）
    let kw: String = name.trim().chars().take(100).collect();
    if kw.is_empty() {
        return Err(DomainError::NotFound(0));
    }
    // 已订阅直接成功——不因愿望单已满把幂等重试演成失败
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM wishlist \
         WHERE user_id = $1 AND keyword = $2)",
    )
    .bind(auth.id)
    .bind(&kw)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if !exists {
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM wishlist WHERE user_id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or(0);
        if count >= 20 {
            return Err(DomainError::Validation(
                "愿望单上限 20 条，请先删除旧的".into(),
            ));
        }
        sqlx::query(
            "INSERT INTO wishlist (user_id, keyword) VALUES ($1, $2) \
             ON CONFLICT (user_id, keyword) DO NOTHING",
        )
        .bind(auth.id)
        .bind(&kw)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(ok(serde_json::json!({ "watching": true, "keyword": kw })))
}
