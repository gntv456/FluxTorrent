//! 作品详情腿（/works/{key}）：works.rs 拆出（300 行门禁）。
//! 列表腿在 works.rs；本文件只保留详情端点 + 键白名单 + 订阅关键词。

use actix_web::{get, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents::promo;
use crate::torrents::works_seed::{self, WorkSeed};

use super::works::WorkRow;
use super::works_rows_sql;
use super::works_sql;

/// 作品详情：头部（作品行）+ 该作品的种子行 + 同分类相关作品。
#[utoipa::path(
    get,
    path = "/api/v1/works/{key}",
    tag = "种子",
    description = "作品详情：作品头 + 作品内种子行（与列表行同形）+ 相关作品。",
    responses(
        (status = 200, description = "ok"),
        (status = 404, description = "作品不存在或无可见种子（code 1004）"),
        (status = 401, description = "会话未登录（code 2001）"),
    )
)]
/// 作品键形态白名单（0424 从 detail 抽公共）：`g`+数字 / `i`+tt… /
/// `m`/`d`+数字 / `n`+32 位十六进制。拼错/乱填直接 404，不放它去跑全表聚合。
pub(crate) fn valid_work_key(key: &str) -> bool {
    let digit =
        |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    match key.as_bytes().first() {
        Some(b'g') => key[1..].parse::<i64>().is_ok(),
        // IMDb 键：`i` + tt…（大小写不敏感的比较交给 SQL，这里只认形态）
        Some(b'i') => key.len() > 3 && key[1..].starts_with(['t', 'T']),
        Some(b'm') | Some(b'd') => digit(&key[1..]),
        Some(b'n') => {
            key.len() == 33 && key[1..].bytes().all(|c| c.is_ascii_hexdigit())
        }
        _ => false,
    }
}

/// 作品订阅关键词（归一化标题）。
///
/// 订阅写（works_watch）与详情回读（work_detail 的 watching）必须取同一
/// 段 SQL 与**同一截断**——各写一份时「订阅成功但刷新又显示未订阅」是必然
/// 结局。愿望单 keyword 口径 1-100 字，超长片名截断必须发生在这一处。
pub(super) async fn keyword_for(
    db: &sqlx::PgPool,
    key: &str,
) -> Result<Option<String>, sqlx::Error> {
    let raw: Option<String> = sqlx::query_scalar(&format!(
        "SELECT COALESCE(NULLIF({title}, ''), t.name) \
         FROM torrents t \
         WHERE t.approval_status = 1 AND {wk} = $1 \
         ORDER BY (t.media_info->>'poster') IS NOT NULL DESC, \
                  t.created_at DESC, t.id DESC LIMIT 1",
        title = works_sql::title_expr(),
        wk = works_sql::wk_expr(),
    ))
    .bind(key)
    .fetch_optional(db)
    .await?;
    Ok(raw.map(|s| s.trim().chars().take(100).collect()))
}

#[get("/works/{key}")]
pub(crate) async fn work_detail(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let key = path.into_inner();
    let valid = valid_work_key(&key);
    if !valid {
        return Err(DomainError::NotFound(0));
    }

    let work: Option<WorkRow> = sqlx::query_as(&works_sql::one_sql())
        .bind(&key)
        .fetch_optional(&state.repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(work) = work else {
        return Err(DomainError::NotFound(0));
    };

    let items: Vec<WorkSeed> = works_seed::for_work(
        &state.repo.read_db,
        &works_rows_sql::rows_sql(&promo::lateral_latest()),
        &key,
    )
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let related: Vec<WorkRow> = sqlx::query_as(&works_sql::related_sql())
        .bind(work.category_id)
        .bind(&key)
        .fetch_all(&state.repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 订阅态回读（0424 P1-2）：按钮刷新后不该退回「订阅新作」。
    // 关键词与订阅写口同一来源（keyword_for），不各写一份。
    let keyword = keyword_for(&state.repo.read_db, &key)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let watching: bool = match keyword {
        Some(kw) => sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM wishlist \
             WHERE user_id = $1 AND keyword = $2)",
        )
        .bind(auth.id)
        .bind(kw)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
        None => false,
    };

    Ok(ok(serde_json::json!({
        "work": work,
        "items": items,
        "related": related,
        "watching": watching,
    })))
}
