//! 封禁公示（成员可见）的取数。
//!
//! 数据现实（10-17 现网核过）：本站**没有**统一的 user_bans/ban_records 表，
//! 处置状态散在四处——`users.status`（0正常/1禁言/2封号，3=注销见
//! admin_http/user_del.rs:32）、`users.warned_until`、
//! `users.cheat_banned_until`（0369 作弊停权），理由文本落在
//! `user_modify_logs.content`（由 admin_http/user_status.rs:160-175 写入
//! 「封禁：xxx」/「禁言：xxx」，无理由时是「状态 → 2」）。
//!
//! 所以这里给的是**状态视图**而不是事件流水：列当前仍处于禁言/封号的账号，
//! 每人取最近一条处置留痕当日期与理由。做成事件流水要留痕本身带结构化的
//! 类型/期限列，现在没有（content 是自由文本），硬解析中文前缀会漂。
//!
//! 隐私：对外结构体 `BanLogRow` 里**只有打码后的用户名**——原始 username
//! 停在私有的 RawRow，映射完即丢，调用方无从把它带出这层；邮箱/IP/passkey
//! 连查都没查。

use crate::errors::{DomainError, DomainResult};
use crate::repo::Repo;

/// 一行公示记录。`status` 1=禁言 2=封号。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct BanLogRow {
    pub uid: i64,
    pub name_masked: String,
    pub status: i16,
    /// 最近一次处置留痕的理由（剥掉中文前缀；无留痕/无理由时为空串）
    pub reason: String,
    /// 处置时间。无留痕时为 None——界面回落「日期不详」，不拿今天顶替。
    #[sqlx(default)]
    pub acted_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    pub warned_until: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    pub cheat_banned_until: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(sqlx::FromRow)]
struct RawRow {
    uid: i64,
    username: String,
    status: i16,
    #[sqlx(default)]
    content: Option<String>,
    #[sqlx(default)]
    acted_at: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    warned_until: Option<chrono::DateTime<chrono::Utc>>,
    #[sqlx(default)]
    cheat_banned_until: Option<chrono::DateTime<chrono::Utc>>,
}

/// 列表与计数共用的谓词：$1 用户名模糊、$2 类型（NULL=全部）。
/// 两处分开写迟早漂——公示计数和列表说的不是同一件事最难查。
const WHERE: &str = " WHERE u.status IN (1, 2) \
    AND ($1::text IS NULL OR u.username ILIKE '%' || $1 || '%') \
    AND ($2::smallint IS NULL OR u.status = $2)";

const LIST_HEAD: &str = concat!(
    "SELECT u.id AS uid, u.username, u.status, m.content, ",
    "m.created_at AS acted_at, u.warned_until, u.cheat_banned_until ",
    "FROM users u LEFT JOIN LATERAL (",
    "SELECT l.content, l.created_at FROM user_modify_logs l ",
    "WHERE l.uid = u.id AND (l.content LIKE '封禁%' ",
    "OR l.content LIKE '禁言%' OR l.content LIKE '状态 → 1%' ",
    "OR l.content LIKE '状态 → 2%') ",
    "ORDER BY l.id DESC LIMIT 1) m ON true",
);

const TAIL: &str = concat!(
    " ORDER BY m.created_at DESC NULLS LAST, u.id DESC",
    " LIMIT $3 OFFSET $4",
);

pub async fn ban_log_page(
    repo: &Repo,
    q: Option<&str>,
    kind: Option<i16>,
    limit: i64,
    offset: i64,
) -> DomainResult<Vec<BanLogRow>> {
    let sql = format!("{LIST_HEAD}{WHERE}{TAIL}");
    let rows: Vec<RawRow> = sqlx::query_as(&sql)
        .bind(q)
        .bind(kind)
        .bind(limit)
        .bind(offset)
        .fetch_all(&repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows
        .into_iter()
        .map(|r| BanLogRow {
            uid: r.uid,
            name_masked: mask_name(&r.username),
            status: r.status,
            reason: strip_label(r.content.as_deref().unwrap_or("")),
            acted_at: r.acted_at,
            warned_until: r.warned_until,
            cheat_banned_until: r.cheat_banned_until,
        })
        .collect())
}

pub async fn ban_log_count(
    repo: &Repo,
    q: Option<&str>,
    kind: Option<i16>,
) -> DomainResult<i64> {
    let n: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM users u{WHERE}"
    ))
    .bind(q)
    .bind(kind)
    .fetch_one(&repo.read_db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n)
}

/// 用户名打码：ASCII 起头留 3 个字符，其余（CJK）留 1 个，尾巴统一 `***`，
/// 与样图 § ban-log 的 `wang***` / `青***` 一致。
pub fn mask_name(name: &str) -> String {
    let cs: Vec<char> = name.chars().collect();
    if cs.is_empty() {
        return "***".to_string();
    }
    let keep = if cs[0].is_ascii_alphanumeric() {
        3.min(cs.len())
    } else {
        1
    };
    let head: String = cs[..keep].iter().collect();
    format!("{head}***")
}

/// 剥掉管理动作的中文前缀（「封禁：多次 H&R」→「多次 H&R」）。
/// 无理由时写入方留的是「状态 → 2」——这种没有可读理由，回空串由界面
/// 显示「未附理由」，不把内部格式串直接甩给用户。
pub fn strip_label(content: &str) -> String {
    for prefix in ["封禁：", "禁言："] {
        if let Some(rest) = content.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    if content.starts_with("状态 →") {
        return String::new();
    }
    content.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::{mask_name, strip_label};

    #[test]
    fn ascii_keeps_three_chars() {
        assert_eq!(mask_name("wangyue"), "wan***");
        assert_eq!(mask_name("ab"), "ab***");
    }

    #[test]
    fn cjk_keeps_one_char() {
        assert_eq!(mask_name("青柠"), "青***");
        assert_eq!(mask_name(""), "***");
    }

    #[test]
    fn label_prefixes_are_stripped() {
        assert_eq!(strip_label("封禁：多次 H&R"), "多次 H&R");
        assert_eq!(strip_label("禁言：灌水 "), "灌水");
        // 内部格式串不得外泄成「理由」
        assert_eq!(strip_label("状态 → 2"), "");
        assert_eq!(strip_label(""), "");
    }
}
