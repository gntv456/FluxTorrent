//! 插件四件套（725 区）：勋章墙/大赛/头像挂件/五子棋。
//! 从 http.rs 按域拆出。

use actix_web::{get, post, web, HttpRequest, Responder};

// auth 模块经 state.jwt 使用（0071 RS256 化后 http 层不再直接调用）

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::auth_infra::require_auth;

// ============ 插件：勋章墙 / 大赛 / 头像挂件 / 五子棋 ============

#[derive(serde::Deserialize)]
struct MedalWallQuery {
    /// 用户名/勋章名模糊搜索（ILIKE，2 字符起搜，与论坛搜索同口径）
    #[serde(default)]
    q: Option<String>,
    #[serde(default = "default_page")]
    page: i64,
    #[serde(default = "default_per_page")]
    per_page: i64,
}

pub fn default_page() -> i64 {
    1
}
pub fn default_per_page() -> i64 {
    12
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct MedalWallUserRow {
    /// 前端跳用户主页用（/users/{id}）：一并带出，省一次按名反查
    user_id: i64,
    username: String,
    medal_name: String,
    asset_ref: Option<String>,
    rarity: Option<String>,
    wearing: bool,
    granted_at: chrono::DateTime<chrono::Utc>,
}

#[derive(serde::Serialize)]
struct MedalWallUser {
    user_id: i64,
    username: String,
    medal_count: i64,
    medals: Vec<MedalWallEntryRich>,
}

#[derive(serde::Serialize)]
struct MedalWallEntryRich {
    medal_name: String,
    asset_ref: Option<String>,
    rarity: Option<String>,
    wearing: bool,
    granted_at: chrono::DateTime<chrono::Utc>,
}

/// 勋章墙（medal_wall.php 口径）：全部用户「持有」的勋章展示墙。
/// 口径=持有且未过期（页面副题「全站用户获得的勋章展示」）；wearing 是单佩戴位
/// （medal_wear 佩戴前会摘掉其余），不能作为墙体过滤——否则每人最多显示一枚。
/// 返回按用户聚合（一卡一人，墙的形态）；q 搜用户名/勋章名（2 字符起）；分页信封与
/// admin 列表同构（rows/total/page/per_page）。行查询 + Rust 线性分组——SQL 侧 jsonb_agg
/// 会让每行都背一遍用户名，且「用户名命中→整人进墙」的搜索语义用 OR 谓词更直白。
#[get("/medal-wall")]
pub async fn medal_wall(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<MedalWallQuery>,
) -> DomainResult<impl Responder> {
    // 搜索语义：q 命中用户名 → 该用户全部勋章都进墙；只命中勋章名 → 只进那几枚。
    // 一条 OR 谓词即两义齐备，无需先查用户再回表。
    // 三段 WHERE 只从「有无关键字」二选一，值不拼接（关键字走 $1 绑定）——format! 仅作模板拼装。
    let has_kw =
        q.q.as_deref()
            .map(str::trim)
            .filter(|s| s.len() >= 2)
            .is_some();
    let pattern = if has_kw {
        Some(crate::http::like_pattern(
            q.q.as_deref().unwrap_or("").trim(),
        ))
    } else {
        None
    };
    let sql_where = if has_kw {
        "WHERE (um.expires_at IS NULL OR um.expires_at > now()) \
         AND (u.username ILIKE $1 OR m.name ILIKE $1)"
    } else {
        "WHERE um.expires_at IS NULL OR um.expires_at > now()"
    };
    // 计数子查询与行查询必须同口径（total 与 rank 窗口来自同一份谓词）——别名带 2 后缀
    let count_where = if has_kw {
        "WHERE (um2.expires_at IS NULL OR um2.expires_at > now()) \
         AND (u2.username ILIKE $1 OR m2.name ILIKE $1)"
    } else {
        "WHERE um2.expires_at IS NULL OR um2.expires_at > now()"
    };
    let (offset, limit) = crate::dto::page_window(q.page, q.per_page);

    // 按「持勋数降序、用户名」的全序给用户编 rank，再按 rank 区间取页窗口——
    // LIMIT 生效在用户维度而非 (user, medal) 行维度（否则一人 10 枚会吃掉整页，
    // 回到「只见一人」的老问题）。
    let rows: Vec<MedalWallUserRow> = sqlx::query_as(&format!(
        "SELECT user_id, username, medal_name, asset_ref, rarity, wearing, granted_at FROM ( \
            SELECT u.id AS user_id, u.username, m.name AS medal_name, m.asset_ref, m.rarity, um.wearing, um.granted_at, m.id AS medal_id, \
                   dense_rank() OVER (ORDER BY agg.cnt DESC, u.username) AS user_rank \
            FROM ( \
                SELECT u2.id AS uid, count(*) AS cnt FROM user_medals um2 \
                JOIN users u2 ON u2.id = um2.user_id JOIN medals m2 ON m2.id = um2.medal_id \
                {count_where} GROUP BY u2.id \
            ) agg JOIN users u ON u.id = agg.uid \
            JOIN user_medals um ON um.user_id = u.id JOIN medals m ON m.id = um.medal_id \
            {row_where} \
         ) ranked WHERE user_rank >= $2 AND user_rank < $2 + $3 ORDER BY user_rank, medal_id",
        row_where = sql_where,
        count_where = count_where,
    ))
    .bind(&pattern)
    .bind(offset + 1) // rank 从 1 起：第 page 页 = rank ∈ [offset+1, offset+limit]
    .bind(limit)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // total = 口径下的用户数（与 rank 窗口同一份 count_where）
    let total_users: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM ( \
            SELECT u2.id FROM user_medals um2 \
            JOIN users u2 ON u2.id = um2.user_id JOIN medals m2 ON m2.id = um2.medal_id \
            {count_where} GROUP BY u2.id \
         ) t",
        count_where = count_where,
    ))
    .bind(&pattern)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // Rust 侧聚合：SQL 已按 user_rank 排序，同用户行连续，一次线性扫描分组
    let mut users: Vec<MedalWallUser> = Vec::new();
    for r in rows {
        let entry = MedalWallEntryRich {
            medal_name: r.medal_name,
            asset_ref: r.asset_ref,
            rarity: r.rarity,
            wearing: r.wearing,
            granted_at: r.granted_at,
        };
        match users.last_mut() {
            Some(u) if u.username == r.username => {
                u.medal_count += 1;
                u.medals.push(entry);
            }
            _ => users.push(MedalWallUser {
                user_id: r.user_id,
                username: r.username,
                medal_count: 1,
                medals: vec![entry],
            }),
        }
    }

    Ok(ok(serde_json::json!({
        "rows": users,
        "total": total_users,
        "page": q.page.max(1),
        "per_page": q.per_page,
    })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct ContestInfo {
    id: i32,
    title: String,
    descr: Option<String>,
    starts_at: chrono::DateTime<chrono::Utc>,
    ends_at: chrono::DateTime<chrono::Utc>,
    is_active: bool,
    entries: i64,
    #[sqlx(default)]
    leader: Option<String>,
    #[sqlx(default)]
    leader_score: Option<i32>,
}

#[get("/contests")]
pub async fn contest_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let mut rows: Vec<ContestInfo> = sqlx::query_as(
        "SELECT c.id, c.title, c.descr, c.starts_at, c.ends_at, c.is_active, \
            (SELECT count(*) FROM contest_entries e WHERE e.contest_id = c.id) AS entries, \
            (SELECT u.username FROM contest_entries e JOIN users u ON u.id = e.user_id \
             WHERE e.contest_id = c.id ORDER BY e.score DESC LIMIT 1) AS leader, \
            (SELECT e.score FROM contest_entries e WHERE e.contest_id = c.id ORDER BY e.score DESC LIMIT 1) AS leader_score \
         FROM contests c ORDER BY c.is_active DESC, c.id DESC",
    ).fetch_all(&state.repo.db).await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let _ = &mut rows;
    Ok(ok(rows))
}

#[post("/contests/{id}/join")]
pub async fn contest_join(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i32>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let inserted = sqlx::query(
        "INSERT INTO contest_entries (contest_id, user_id) VALUES ($1, \
         $2) ON CONFLICT DO NOTHING",
    )
    .bind(*path)
    .bind(auth.id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if inserted.rows_affected() == 0 {
        return Err(DomainError::Validation("已报名".into()));
    }
    Ok(ok(serde_json::json!({ "joined": true })))
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub(super) struct FrameRow {
    pub(super) id: i32,
    pub(super) name: String,
    pub(super) css: String,
    #[sqlx(default)]
    pub(super) image_url: Option<String>,
    pub(super) price: i32,
}
