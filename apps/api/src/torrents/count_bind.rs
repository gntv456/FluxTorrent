//! 列表计数侧 bind 链唯一真源（批一百五十二修复）。
//!
//! 背景：`hidden_dead` 落地时 `all_sql` 手抄了一份 `count_sql` 的 24 项
//! bind 链，漏绑了 `$6`（标题 pattern），导致参数槽位整体错位、PG 报
//! syntax error、列表接口现网 500。手抄两份 = 迟早再错一次。
//!
//! 现在两条计数 SQL 共用本模块的 `CountArgs` + `bind_count_args`：
//! 槽位编号与绑定顺序写在同一张表里，加谓词只改一处。
//!
//! 槽位（计数侧，两条 SQL 共用同一编号——`all_sql` 只是少一个存活谓词，
//! 不改编号）：
//!   $1 category_id  $2 medium_id  $3 grade_id  $4 edition_id
//!   $5 official     $6 pattern(标题/关键字)  $7 include_unapproved
//!   $8 tag_ids      $9 size_min  $10 size_max  $11 date_from  $12 date_to
//!   $13 min_seeders $14 max_seeders $15 exclude $16 promo $17 owner
//!   $18 mine(viewer) $19 min_leechers $20 max_leechers
//!   $21 min_completed $22 max_completed $23 anonymous
//!   $24 title_like   $25 rating_min

use super::list::esc_like;
use super::types::TorrentFilter;
use sqlx::postgres::PgArguments;
use sqlx::{Arguments, PgPool};

/// 计数侧 25 个绑定值（一次性构造，供两条计数 SQL 复用）。
#[derive(Debug, Clone)]
pub(super) struct CountArgs {
    category_id: Option<Vec<i32>>,
    medium_id: Option<i32>,
    grade_id: Option<i32>,
    edition_id: Option<i32>,
    official: Option<bool>,
    pattern: Option<String>,
    include_unapproved: bool,
    tag_ids: Option<Vec<i32>>,
    size_min: Option<i64>,
    size_max: Option<i64>,
    date_from: Option<String>,
    date_to: Option<String>,
    min_seeders: Option<i32>,
    max_seeders: Option<i32>,
    exclude_pat: Option<String>,
    promo: Option<String>,
    owner_pat: Option<String>,
    mine: Option<i64>,
    min_leechers: Option<i32>,
    max_leechers: Option<i32>,
    min_completed: Option<i32>,
    max_completed: Option<i32>,
    anonymous: Option<i16>,
    title_pat: Option<String>,
    rating_min: Option<f64>,
}

impl CountArgs {
    /// 从 filter + 已算好的 pattern 构造（pattern 由调用方按 search_mode
    /// 决定精确/模糊，title_like 永远模糊——口径见 list.rs `extra_preds`）。
    pub(super) fn new(
        filter: &TorrentFilter,
        pattern: Option<&str>,
        viewer: i64,
    ) -> Self {
        let fuzz = |v: &Option<String>| {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| format!("%{}%", esc_like(s)))
        };
        Self {
            category_id: filter.category_id.clone(),
            medium_id: filter.medium_id,
            grade_id: filter.grade_id,
            edition_id: filter.edition_id,
            official: filter.official,
            pattern: pattern.map(|s| s.to_string()),
            include_unapproved: filter.include_unapproved,
            tag_ids: filter.tag_ids.clone(),
            size_min: filter.size_min,
            size_max: filter.size_max,
            date_from: filter.date_from.clone(),
            date_to: filter.date_to.clone(),
            min_seeders: filter.min_seeders,
            max_seeders: filter.max_seeders,
            exclude_pat: fuzz(&filter.exclude),
            promo: filter.promo.clone(),
            owner_pat: fuzz(&filter.owner),
            mine: filter.only_mine.then_some(viewer),
            min_leechers: filter.min_leechers,
            max_leechers: filter.max_leechers,
            min_completed: filter.min_completed,
            max_completed: filter.max_completed,
            anonymous: filter.anonymous,
            title_pat: fuzz(&filter.title_like),
            rating_min: filter.rating_min,
        }
    }
}

/// 把 25 项按槽位顺序压进 `PgArguments`。
///
/// 用 `PgArguments::add` 逐项 push（它按 push 顺序分配 $1..$25），保证
/// 「谁写的槽位编号」和「谁 push 的顺序」是同一张表里的同一行——
/// 这正是原 bug 的根因（编号写在 format! 里，顺序写在 .bind 链里，两处独立）。
macro_rules! add {
    ($args:expr, $v:expr) => {{
        $args.add($v).expect("计数 bind 编码不应失败（值均为 SQLx 支持类型）");
    }};
}

pub(super) fn count_args(args: &CountArgs) -> PgArguments {
    let mut a = PgArguments::default();
    add!(a, &args.category_id);
    add!(a, args.medium_id);
    add!(a, args.grade_id);
    add!(a, args.edition_id);
    add!(a, args.official);
    add!(a, &args.pattern);
    add!(a, args.include_unapproved);
    add!(a, &args.tag_ids);
    add!(a, args.size_min);
    add!(a, args.size_max);
    add!(a, &args.date_from);
    add!(a, &args.date_to);
    add!(a, args.min_seeders);
    add!(a, args.max_seeders);
    add!(a, &args.exclude_pat);
    add!(a, &args.promo);
    add!(a, &args.owner_pat);
    add!(a, args.mine);
    add!(a, args.min_leechers);
    add!(a, args.max_leechers);
    add!(a, args.min_completed);
    add!(a, args.max_completed);
    add!(a, args.anonymous);
    add!(a, &args.title_pat);
    add!(a, args.rating_min);
    a
}

/// 计数 SQL 执行器：两条 SQL 共用，签名固定为 count 标量。
pub(super) async fn fetch_count(
    sql: &str,
    args: &CountArgs,
    db: &PgPool,
) -> sqlx::Result<i64> {
    let a = count_args(args);
    sqlx::query_scalar_with(sql, a).fetch_one(db).await
}
#[cfg(test)]
mod tests {
    use super::*;

    fn empty_filter() -> TorrentFilter {
        TorrentFilter::default()
    }

    /// 槽位数判据（批一百五十二真洞）：两条计数 SQL 共用 `CountArgs`，
    /// 绑定个数必须等于计数侧最后一个槽位（rating_min = $25）。
    /// 漏绑任意一项 → 全部槽位右移 → PG syntax error（现网 500）。
    #[test]
    fn bind_chain_covers_every_slot() {
        let f = empty_filter();
        let args = CountArgs::new(&f, None, 42);
        let a = count_args(&args);
        // PgArguments 记录每个绑定的类型；个数即绑定项数。
        assert_eq!(
            a.len(),
            25,
            "计数侧应恰好绑定 25 项（$1..$25），少一项即槽位错位"
        );
    }

    /// 非默认 filter 走同一条链：谓词多开不影响绑定个数（谓词按
    /// `$n IS NULL` 自适应，不额外占槽）。
    #[test]
    fn bind_count_is_filter_independent() {
        let mut f = empty_filter();
        f.search = Some("abc".into());
        f.exclude = Some("def".into());
        f.owner = Some("me".into());
        f.title_like = Some("S01".into());
        f.only_mine = true;
        f.size_min = Some(1);
        f.rating_min = Some(8.0);
        let args = CountArgs::new(&f, Some("%abc%"), 7);
        let a = count_args(&args);
        assert_eq!(a.len(), 25, "谓词全开时绑定数仍是 25");
    }

    /// $18（仅我发布）必须绑 viewer，且 viewer 参与编码——
    /// 原 bug 里这一项曾被 `.then_some(viewer)` 误写成固定值。
    #[test]
    fn only_mine_carries_viewer() {
        let mut f = empty_filter();
        f.only_mine = true;
        let args = CountArgs::new(&f, None, 12345);
        assert_eq!(args.mine, Some(12345));
        let off = CountArgs::new(
            &{
                let mut g = empty_filter();
                g.only_mine = false;
                g
            },
            None,
            12345,
        );
        assert_eq!(off.mine, None, "未开启仅我发布时绑 NULL");
    }

    /// 模糊转义口径一致：title_like / exclude / owner 都走 `esc_like`
    /// （% 与 _ 不再当通配符），与列表侧 `extra_preds` 对齐。
    #[test]
    fn fuzzy_patterns_are_escaped() {
        let mut f = empty_filter();
        f.exclude = Some("50%_off".into());
        let args = CountArgs::new(&f, None, 1);
        assert_eq!(args.exclude_pat.as_deref(), Some("%50\\%\\_off%"));
    }
}
