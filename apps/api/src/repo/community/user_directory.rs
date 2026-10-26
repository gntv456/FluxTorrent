//! 会员列表（GET /users）的 SQL 居所。
//!
//! 为什么落在 `repo/community/` 而不是 `repo/` 根：`repo/mod.rs` 已压在
//! 302 行基线上（只许瘦不许胖），再加一行模块声明就顶红门禁；
//! 会员目录是「人」的社区面读口，与 profile/follows 同域，就近安家。
//!
//! 2026-10-17 随「用户列表对齐样图」新增。此前全站唯一的用户列表是
//! `GET /admin/users`（`staff` + `USER_VIEW` 双门禁），前台 `/users` 只能
//! `redirect("/top")` 拿排行榜顶替——样图那张 10 列会员表一直没有读口。
//!
//! ⚠️ 三条硬口径，改这里之前先读：
//! 1. **绝不投影 `email`**：`/admin/users` 会回邮箱（管理端要按邮箱查人），
//!    公开列表一旦带上就是全站批量泄邮箱。0233/隐私批已经把
//!    `/users/{id}` 与 `/users/{id}/torrentlist` 的邮箱收掉，这里不能放开。
//! 2. **清号行必须滤掉**：硬注销（`admin_http/user_del.rs`）不删行，只把
//!    身份就地改写成 `deleted-<id>-<hash>` / `@deleted.invalid`，
//!    不过滤就会把注销账号排成真实会员。
//! 3. **封号/封存/归档不进列表**：`status >= 2`、`parked`、`archived`
//!    三类都不是「可被浏览的会员」；禁言（`status = 1`）仍在，
//!    因为禁言者依然是成员，只是不能发言。

use super::super::Repo;
use crate::errors::{DomainError, DomainResult};

/// 一行的读侧视图（字段名即 JSON 名，前端逐列对）。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct MemberRow {
    pub id: i64,
    pub username: String,
    pub title: Option<String>,
    pub avatar_url: Option<String>,
    pub class_id: i32,
    pub class_name: Option<String>,
    pub uploaded: i64,
    pub downloaded: i64,
    /// 正在做种数（`snatches.seeding`），样图「做种」列
    pub seeding: i64,
    pub spark_balance: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 最近上线；老数据可能为 NULL，前端显「—」
    pub last_seen_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// 排序白名单：外部串一律先过这里，不拼进 SQL 的原始输入。
pub fn order_col(raw: Option<&str>) -> &'static str {
    match raw.unwrap_or("created") {
        "id" => "u.id",
        "class" => "u.class_id",
        "uploaded" => "u.uploaded",
        "downloaded" => "u.downloaded",
        "spark" => "u.spark_balance",
        "last_seen" => "last_seen_at",
        "seeding" => "seeding",
        _ => "u.created_at",
    }
}

/// 名录型出口共用的「在册会员」谓词（本模块顶部规则 3：
/// 封号/封存/归档/墓碑不进列表）。别名由调用方给：名录与荣誉榜用 `u`，
/// 勋章墙的排名子查询用 `u2`。
///
/// P-529/P-530：荣誉榜与勋章墙此前各写一份 WHERE、两处都漏了这四项——
/// 现网实测 `/pool/honor` 5 席里 4 席、`/medal-wall` 27 个名字里 10 个是
/// `status >= 2` 或 `deleted-` 墓碑号（名录自己是对的，漏的是邻居）。
/// 单点放在这里，和名录共用一份字面量，免得再分叉。
pub(crate) fn member_visible(alias: &str) -> String {
    let a = format!("{}.", alias);
    format!(
        "{}status < 2 AND NOT {}parked AND NOT {}archived \
             AND {}username NOT LIKE 'deleted-%'",
        a, a, a, a
    )
}

/// 会员名录专用：在册四条件 + 可空的关键词检索（`$1`）。
fn visible() -> String {
    format!(
        "{} AND ($1::text IS NULL \
             OR u.username::text ILIKE '%' || $1 || '%')",
        member_visible("u")
    )
}

#[allow(clippy::too_many_arguments)]
pub async fn members(
    repo: &Repo,
    q: Option<&str>,
    class_id: Option<i32>,
    order: &str,
    desc: bool,
    limit: i64,
    offset: i64,
) -> DomainResult<Vec<MemberRow>> {
    // class_id 是可选谓词，只能拼字符串（值本身走 $2 绑定）；
    // order/dir 来自 order_col 白名单与一个布尔，无注入面。
    let class_filter = if class_id.is_some() {
        " AND u.class_id = $2"
    } else {
        ""
    };
    let sql = {
        // P-458：PG 的 `DESC` 缺省是 **NULLS FIRST**，而 `users.last_seen_at`
        // 可空。会员目录的排序档 `desc` 缺省为 true，于是点「最近上线」拿到的
        // 第一页全是**从没上线过**的号（现网实测 20/20 行为 NULL，56 个可见
        // 会员里 50 个 last_seen_at 为空），真最近活跃的 6 个人被推到第三页
        // 之后——「最近上线」这一档排出来的是它的反面。
        // 两个方向一律 NULLS LAST：未知值沉底。非空列（本表其余排序键都非空）
        // 加它零影响；同仓已有先例 `openapi_http/me_torrents.rs`、
        // `staff_http/site_db.rs` 写的就是 `DESC NULLS LAST`。
        let ord = if desc {
            "DESC NULLS LAST"
        } else {
            "ASC NULLS LAST"
        };
        let (lim, off) = if class_id.is_some() {
            ("$3", "$4")
        } else {
            ("$2", "$3")
        };
        let vis = visible();
        format!(
            "SELECT u.id, u.username, u.title, u.avatar_url, u.class_id, \
                c.name AS class_name, u.uploaded, u.downloaded, \
                (SELECT count(*) FROM snatches s \
                  WHERE s.user_id = u.id AND s.seeding) AS seeding, \
                u.spark_balance, u.created_at, u.last_seen_at \
             FROM users u LEFT JOIN user_classes c ON c.id = u.class_id \
             WHERE {vis}{class_filter} \
             ORDER BY {order} {ord}, u.id {ord} \
             LIMIT {lim} OFFSET {off}"
        )
    };
    // 关键词按 esc_like 口径转义（P-332）：裸 %/_ 进 ILIKE 会变成通配
    let q = q.map(crate::torrents::esc_like);
    let mut query = sqlx::query_as::<_, MemberRow>(&sql).bind(q);
    if let Some(cid) = class_id {
        query = query.bind(cid);
    }
    query
        .bind(limit)
        .bind(offset)
        .fetch_all(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}

/// 总数与列表同一套 WHERE，否则前端页码会数出一批空页。
pub async fn count_members(
    repo: &Repo,
    q: Option<&str>,
    class_id: Option<i32>,
) -> DomainResult<i64> {
    let class_filter = if class_id.is_some() {
        " AND u.class_id = $2"
    } else {
        ""
    };
    let vis = visible();
    let sql = format!("SELECT count(*) FROM users u WHERE {vis}{class_filter}");
    // 列表与计数同一谓词，转义也必须同一处（P-332）
    let q = q.map(crate::torrents::esc_like);
    let mut query = sqlx::query_scalar::<_, i64>(&sql).bind(q);
    if let Some(cid) = class_id {
        query = query.bind(cid);
    }
    query
        .fetch_one(&repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}
