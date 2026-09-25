//! 列表游标（keyset）构造：排序键枚举、游标谓词、下一页游标编码。
//! 从 list_noclamp_as.rs 按域拆出（300 行门禁）。
//!
//! 语义（方案批次二）：用户显式排序**不掺置顶**——「按做种数排序」就该纯按
//! 做种数，置顶只在默认浏览顺序生效（NP 同口径）；这也是排序能走 (col, id)
//! 复合索引的前提（sticky 表达式含 now()，属 STABLE，掺进 ORDER BY 就建不了索引）。
//! 游标从「只比 id」升级为「排序列值 + id」二元 keyset，修复非默认排序下
//! 「id 更大但排得更前」的种子永远翻不出来的缺陷。

use super::types::{ListCursor, TorrentRow};

/// 排序键取值的函数指针（next_cursor 编码用，避免按列名再 match 行字段）
pub(super) type RowFn = fn(&TorrentRow) -> String;

/// 游标键的「排序表达式 + cast + 行取值函数」三元组：
/// 谓词比较用前两者，next_cursor 编码用第三个。
pub(super) enum CursorCol {
    /// 默认排序：置顶权重 + id
    Sticky,
    Int(&'static str, &'static str, RowFn),
    Big(&'static str, &'static str, RowFn),
    Ts(&'static str, &'static str, RowFn),
    Text(&'static str, &'static str, RowFn),
    /// 表达式列（comments 计数）：expr 即子查询文本
    ExprInt(&'static str, &'static str, RowFn),
}

fn dir(asc: bool) -> &'static str {
    if asc {
        "ASC"
    } else {
        "DESC"
    }
}

/// 构造 (ORDER BY 片段, 游标键)。`sticky_expr` 已含 DESC，仅默认排序用。
pub(super) fn sort_of(
    key: &str,
    asc: bool,
    sticky_expr: &str,
) -> (String, CursorCol) {
    let d = dir(asc);
    let plain = |col: &str| format!("{col} {d}, t.id {d}");
    match key {
        "seeders" => (
            plain("t.seeders"),
            CursorCol::Int("t.seeders", "int", |r: &TorrentRow| {
                r.seeders.to_string()
            }),
        ),
        "leechers" => (
            plain("t.leechers"),
            CursorCol::Int("t.leechers", "int", |r: &TorrentRow| {
                r.leechers.to_string()
            }),
        ),
        "size" => (
            plain("t.size"),
            CursorCol::Big("t.size", "bigint", |r: &TorrentRow| {
                r.size.to_string()
            }),
        ),
        "completed" => (
            plain("t.times_completed"),
            CursorCol::Int("t.times_completed", "int", |r: &TorrentRow| {
                r.times_completed.to_string()
            }),
        ),
        // 发布时间/名称排序（高级搜索排序扩展；NP「按发布时间/标题」口径）
        "created" => (
            plain("t.created_at"),
            CursorCol::Ts("t.created_at", "timestamptz", |r: &TorrentRow| {
                r.created_at.to_rfc3339()
            }),
        ),
        "name" => (
            plain("t.name"),
            CursorCol::Text("t.name", "text", |r: &TorrentRow| r.name.clone()),
        ),
        "comments" => (
            plain(
                "(SELECT count(*) FROM comments c \
                 WHERE c.torrent_id = t.id)",
            ),
            CursorCol::ExprInt(
                "(SELECT count(*) FROM comments c WHERE c.torrent_id = t.id)",
                "int",
                |r: &TorrentRow| r.comments.to_string(),
            ),
        ),
        // 0170：方向吃 asc 参数 —— dir=prev 反向翻页时 id 比较方向随之反转
        _ => (format!("{sticky_expr}, t.id {d}"), CursorCol::Sticky),
    }
}

/// 游标谓词：$7 = 游标 id，$26 = 排序键值（文本进、按列 cast 比较）。
/// 与 ORDER BY 完全同序；键值为 NULL（旧格式游标）时短路 → 回到第一页。
/// `tie_inclusive`（0170 双向翻页）：dir=prev 反向页传 true —— 锚行（正向页
/// 末行）属于上一页，tie-break 须含等（id >= $7），否则回翻会丢锚行。
pub(super) fn predicate(
    col: &CursorCol,
    asc: bool,
    sticky_calc: &str,
    tie_inclusive: bool,
) -> String {
    let cmp = if asc { ">" } else { "<" };
    let tie = if tie_inclusive {
        if asc {
            ">="
        } else {
            "<="
        }
    } else {
        cmp
    };
    let (expr, cast) = match col {
        CursorCol::Sticky => (sticky_calc.to_string(), "int"),
        CursorCol::Int(x, c, _) => ((*x).to_string(), *c),
        CursorCol::Big(x, c, _) => ((*x).to_string(), *c),
        CursorCol::Ts(x, c, _) => ((*x).to_string(), *c),
        CursorCol::Text(x, c, _) => ((*x).to_string(), *c),
        CursorCol::ExprInt(x, c, _) => ((*x).to_string(), *c),
    };
    format!(
        " AND (${v}::{cast} IS NULL OR ({expr} {cmp} ${v}::{cast} \
         OR ({expr} = ${v}::{cast} AND t.id {tie} $7)))",
        v = 26,
        cast = cast,
        expr = expr
    )
}

/// 下一页游标编码（排序键值 + id）
pub(super) fn encode_next(col: &CursorCol, row: &TorrentRow) -> String {
    let val = match col {
        CursorCol::Sticky => row.sticky_rank.to_string(),
        CursorCol::Int(_, _, f)
        | CursorCol::Big(_, _, f)
        | CursorCol::Ts(_, _, f)
        | CursorCol::Text(_, _, f)
        | CursorCol::ExprInt(_, _, f) => f(row),
    };
    ListCursor::encode(Some(&val), row.id)
}
