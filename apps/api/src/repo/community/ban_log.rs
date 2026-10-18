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
//! 每人取最近一条处置留痕当「处置日期」。做成事件流水要留痕本身带结构化的
//! 类型/期限列，现在没有（content 是自由文本），硬解析中文前缀会漂。
//!
//! 三条隐私口径（10-17 复审时收紧，前一版把内部备注原文当公示理由发了出去）：
//! 1. 对外只有**打码后的用户名**——原始 username 停在私有 RawRow，映射完即丢；
//! 2. **不发 uid**：打码是为了不让人对号入座，带一个稳定主键等于把「对号」的
//!    键白送出去，而界面本来也不需要它（列表键用行序）；
//! 3. **不发留痕正文**：`content` 是管理组自由文本，可能含邮箱/IP/他人用户名。
//!    公示只给类别码（cheat/mute/admin），详细理由由 admin_http/user_status.rs
//!    走 PM + 邮件单独发给当事人——站内既有口径就写着「仅展示状态与时间」。
//! 邮箱/IP/passkey 连查都没查。

use crate::errors::{DomainError, DomainResult};
use crate::repo::Repo;

/// 一行公示记录。`status` 1=禁言 2=封号；`cause` 是**类别码**
/// （cheat | mute | admin），不是管理组原文——见文件头隐私口径第 3 条。
#[derive(sqlx::FromRow, serde::Serialize)]
pub struct BanLogRow {
    pub name_masked: String,
    pub status: i16,
    pub cause: String,
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
    username: String,
    status: i16,
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

/// 处置时间只取「哪一条留痕最近」，**不取留痕正文**（理由文本见文件头③）。
const LIST_HEAD: &str = concat!(
    "SELECT u.username, u.status, m.created_at AS acted_at, ",
    "u.warned_until, u.cheat_banned_until ",
    "FROM users u LEFT JOIN LATERAL (",
    "SELECT l.created_at FROM user_modify_logs l ",
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
        // 关键词按 esc_like 口径转义（P-332）：裸 %/_ 进 ILIKE 会变成通配
        .bind(q.map(crate::torrents::esc_like))
        .bind(kind)
        .bind(limit)
        .bind(offset)
        .fetch_all(&repo.read_db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows
        .into_iter()
        .map(|r| BanLogRow {
            name_masked: mask_name(&r.username),
            status: r.status,
            cause: cause_of(r.status, r.cheat_banned_until.is_some()),
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
    let n: i64 =
        sqlx::query_scalar(&format!("SELECT count(*) FROM users u{WHERE}"))
            // 与列表同一谓词同一转义（P-332）
            .bind(q.map(crate::torrents::esc_like))
            .bind(kind)
            .fetch_one(&repo.read_db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(n)
}

/// 用户名打码：ASCII 起头留 3 个字符、其余（CJK）留 1 个，尾巴统一 `***`，
/// 与样图 § ban-log 的 `wang***` / `青***` 一致。
///
/// ⚠️ 短名字必须少留一位：`lee`（3 字符）若照「留 3 个」原样输出就是
/// `lee***`＝全名，打码白做。所以保留数封顶到 `len-1`，永远至少藏住一个字符。
pub fn mask_name(name: &str) -> String {
    let cs: Vec<char> = name.chars().collect();
    if cs.is_empty() {
        return "***".to_string();
    }
    let want = if cs[0].is_ascii_alphanumeric() { 3 } else { 1 };
    let keep = want.min(cs.len().saturating_sub(1)).max(1);
    let head: String = cs[..keep].iter().collect();
    format!("{head}***")
}

/// 公示用的**处置类别码**（界面按码出三语文案）。
///
/// 为什么不发留痕原文：`user_modify_logs.content` 是管理组的内部备注自由文本
/// （admin_http/user_status.rs 写的是「封禁：<站长随手写的理由>」），里面完全
/// 可能出现邮箱、IP、别的用户名或案情细节——那是给当事人和职员看的，不是公示栏
/// 的内容。站内口径本就写着「出于隐私考虑，仅展示状态与时间」
/// （`dict.banlog.note`，且当事人由 PM+邮件单独收到理由），所以这里只给类别。
pub fn cause_of(status: i16, cheat_timed: bool) -> String {
    if cheat_timed {
        return "cheat".to_string();
    }
    if status == 1 {
        return "mute".to_string();
    }
    "admin".to_string()
}

#[cfg(test)]
mod tests {
    use super::{cause_of, mask_name};

    #[test]
    fn ascii_keeps_three_chars() {
        assert_eq!(mask_name("wangyue"), "wan***");
        assert_eq!(mask_name("wang"), "wan***");
    }

    #[test]
    fn short_names_never_leak_whole_name() {
        // 全名 3 字符 / 2 字符 / 1 字符：藏住至少一位
        assert_eq!(mask_name("lee"), "le***");
        assert_eq!(mask_name("ab"), "a***");
        assert_eq!(mask_name("a"), "a***");
        assert_eq!(mask_name(""), "***");
    }

    #[test]
    fn cjk_keeps_one_char() {
        assert_eq!(mask_name("青柠"), "青***");
        // 单字 CJK 名也只能留一位（等于无从再藏），但不会多留
        assert_eq!(mask_name("青"), "青***");
    }

    #[test]
    fn cause_codes_are_structured() {
        assert_eq!(cause_of(2, true), "cheat");
        assert_eq!(cause_of(2, false), "admin");
        assert_eq!(cause_of(1, false), "mute");
    }
}
