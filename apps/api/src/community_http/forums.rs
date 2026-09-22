//! M15 论坛版块（forum_list/forum_search）。
//! 从 community_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use super::*;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct ForumRow {
    id: i64,
    name: String,
    descr: Option<String>,
    topics: i64,
    posts: i64,
    #[sqlx(default)]
    latest_topic: Option<String>,
    #[sqlx(default)]
    latest_author: Option<String>,
    #[sqlx(default)]
    latest_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 分区/节点（0115）：首页按分类分组
    #[sqlx(default)]
    category_id: Option<i64>,
    #[sqlx(default)]
    category_name: Option<String>,
    can_write: bool,
    can_create: bool,
    can_mod: bool,
}

#[derive(Deserialize)]
struct ForumSearchQuery {
    q: String,
}

/// 论坛搜索（NP 顶栏搜索搜帖口径，Phase3 增强）：标题 OR 正文纯文本（body_text，0115 落列）
/// 命中正文时带回摘要片段（关键词前后各 ~30 字符，前端高亮用）。
/// 权限：与列表同套谓词（minclassread / 版主），无权版块的内容搜不到。
#[get("/forums/search")]
async fn forum_search(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<ForumSearchQuery>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let kw = q.q.trim();
    if kw.len() < 2 {
        return Err(DomainError::Validation("关键字至少 2 个字符".into()));
    }
    // 转义 LIKE 通配符（与种子搜索同口径）
    let esc_kw = kw
        .replace("\\", "\\\\")
        .replace("%", "\\%")
        .replace("_", "\\_");
    let pattern = format!("%{esc_kw}%");
    // 每主题取「标题命中优先，否则正文最早命中」的楼层做片段；t.id 去重保一主题一行
    let rows: Vec<(i64, String, i64, String, Option<String>, chrono::DateTime<chrono::Utc>, i64, bool, Option<String>)> =
        sqlx::query_as(
            "SELECT t.id, t.title, f.id, f.name, u.username, t.created_at, \
                    (SELECT count(*) FROM posts p WHERE p.topic_id = t.id), t.locked, \
                    (SELECT left(p2.body_text, 90) FROM posts p2 WHERE p2.topic_id = t.id \
                      AND p2.body_text ILIKE $1 ORDER BY p2.id LIMIT 1) AS snippet \
             FROM topics t \
             JOIN forums f ON f.id = t.forum_id \
             LEFT JOIN users u ON u.id = t.user_id \
             WHERE (t.title ILIKE $1 OR EXISTS (SELECT 1 FROM posts p3 WHERE p3.topic_id = t.id AND p3.body_text ILIKE $1)) \
               AND (f.minclassread <= $2 OR EXISTS (SELECT 1 FROM forum_mods fm WHERE fm.forum_id = f.id AND fm.user_id = $3)) \
             ORDER BY t.id DESC LIMIT 30",
        )
        .bind(&pattern)
        .bind(auth.class_id)
        .bind(auth.id)
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<serde_json::Value> = rows
        .iter()
        .map(
            |(tid, title, fid, fname, author, at, replies, locked, snippet)| {
                serde_json::json!({
                    "topic_id": tid, "title": title, "forum_id": fid, "forum_name": fname,
                    "author": author, "created_at": at.to_rfc3339(),
                    "replies": replies, "locked": locked,
                    "snippet": snippet, "keyword": kw,
                })
            },
        )
        .collect();
    Ok(ok(items))
}

/// 分区行（前台索引用）。具名结构体，不用元组——元组会被序列化成数组的数组，
/// 前端按字段取值会静默拿到 undefined。
#[derive(sqlx::FromRow, serde::Serialize)]
struct ForumCatRow {
    id: i64,
    name: String,
    sort: i32,
    visible: bool,
    /// 该分区下**当前用户可读**的版块数（与 forums 列表同套谓词，避免计数与列表不一致）
    forums: i64,
}

/// 版块精简行（移动主题下拉用）
#[derive(sqlx::FromRow, serde::Serialize)]
struct BoardBrief {
    id: i64,
    name: String,
    category_name: Option<String>,
}

#[get("/forums")]
async fn forum_list(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let is_staff = auth.class_id >= 93;
    // 分区列表：**含空分区**。前台据此渲染分组与空态占位——
    // 若像旧版那样从版块列表反推分组，新建的空分区在前台会整块消失。
    let cats: Vec<ForumCatRow> = sqlx::query_as(
        "SELECT c.id, c.name, c.sort, c.visible, \
            (SELECT count(*) FROM forums f \
              WHERE f.category_id = c.id \
                AND (f.minclassread <= $1 OR EXISTS ( \
                  SELECT 1 FROM forum_mods fm \
                   WHERE fm.forum_id = f.id AND fm.user_id = $2))) AS forums \
         FROM forum_categories c \
         WHERE c.visible = TRUE OR $3 \
         ORDER BY c.sort, c.id",
    )
    .bind(auth.class_id)
    .bind(auth.id)
    .bind(is_staff)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 版块列表：只列本版块门槛允许（minclassread）+ 版主兼任的版块；
    // 按 分区 sort → 版块 sort → id 排序，保证同一分区内聚。
    let rows = sqlx::query_as::<
        _,
        (
            i64,
            String,
            Option<String>,
            i64,
            i64,
            Option<i64>,
            Option<String>,
        ),
    >(
        "SELECT f.id, f.name, f.descr, \
            (SELECT count(*) FROM topics t WHERE t.forum_id = f.id) AS topics, \
            (SELECT count(*) FROM posts p JOIN topics t ON t.id = p.topic_id \
              WHERE t.forum_id = f.id) AS posts, \
            f.category_id, c.name AS category_name \
         FROM forums f LEFT JOIN forum_categories c ON c.id = f.category_id \
         WHERE (f.minclassread <= $1 OR EXISTS ( \
                 SELECT 1 FROM forum_mods fm \
                  WHERE fm.forum_id = f.id AND fm.user_id = $2)) \
           AND (c.id IS NULL OR c.visible = TRUE OR $3) \
         ORDER BY c.sort NULLS LAST, c.id NULLS LAST, f.sort, f.id",
    )
    .bind(auth.class_id)
    .bind(auth.id)
    .bind(is_staff)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    let mut latest: std::collections::HashMap<
        i64,
        (
            Option<String>,
            Option<String>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    > = std::collections::HashMap::new();
    let latest_rows: Vec<(
        i64,
        Option<String>,
        Option<String>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = sqlx::query_as(
        "SELECT DISTINCT ON (t.forum_id) t.forum_id, t.title, u.username, t.created_at \
         FROM topics t LEFT JOIN users u ON u.id = t.user_id \
         WHERE t.forum_id = ANY(SELECT id FROM forums WHERE minclassread <= $1) \
         ORDER BY t.forum_id, t.id DESC",
    )
    .bind(auth.class_id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    for (fid, title, author, at) in latest_rows {
        latest.insert(fid, (title, author, at));
    }

    let mut out = Vec::with_capacity(rows.len());
    for (id, name, descr, topics, posts, category_id, category_name) in rows {
        let perm =
            forum_access(&state.repo.db, auth.id, auth.class_id, id).await?;
        let (latest_topic, latest_author, latest_at) =
            latest.get(&id).cloned().unwrap_or((None, None, None));
        out.push(ForumRow {
            id,
            name,
            descr,
            topics,
            posts,
            latest_topic,
            latest_author,
            latest_at,
            category_id,
            category_name,
            can_write: perm.can_write,
            can_create: perm.can_create,
            can_mod: perm.can_mod,
        });
    }
    Ok(ok(serde_json::json!({ "categories": cats, "forums": out })))
}

/// 全站版块精简列表（「移动主题到…」下拉的数据源）。
/// 口径与 forum_list 的版块过滤一致：只回当前用户可读的版块，不泄漏隐藏版块名。
#[get("/forums/boards")]
async fn forum_boards(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let is_staff = auth.class_id >= 93;
    let rows: Vec<BoardBrief> = sqlx::query_as(
        "SELECT f.id, f.name, c.name AS category_name \
         FROM forums f LEFT JOIN forum_categories c ON c.id = f.category_id \
         WHERE (f.minclassread <= $1 OR EXISTS ( \
                 SELECT 1 FROM forum_mods fm \
                  WHERE fm.forum_id = f.id AND fm.user_id = $2)) \
           AND (c.id IS NULL OR c.visible = TRUE OR $3) \
         ORDER BY c.sort NULLS LAST, c.id NULLS LAST, f.sort, f.id",
    )
    .bind(auth.class_id)
    .bind(auth.id)
    .bind(is_staff)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}
