//! 列表计数侧 bind 链唯一真源（批一百五十二修复）。
//!
//! 背景：`all_sql` 曾手抄 count_sql 的 24 项 bind 链漏绑 $6 → 槽位错位
//! → 现网 500。两条计数 SQL 共用 `CountArgs`（0468 起再合并进 counts_for
//! 的同一模板），槽位编号与绑定顺序写在同一张表，加谓词只改一处。
//! 槽位：$1 cat $2 med $3 grade $4 edit $5 official $6 pattern
//! $7 unapproved $8 tags $9-14 size/date/seeders $15 exclude $16 promo
//! $17 owner $18 mine $19-22 leech/complete $23 anon $24 title $25 rating

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

/// 计数总入口（自 list_noclamp_as 迁入）：total + hidden_dead 一次算完。
/// 谓词串由列表侧原样传入（编号/口径与列表 SQL 同源，见调用方）；
/// cap 语义同 cap_clause。失败降级 0（此前口径：留日志不留 500）。
pub(super) async fn counts_for(
    db: &PgPool,
    args: &CountArgs,
    preds: &CountPreds,
    cap: i64,
) -> (i64, i64) {
    // 解包谓词：format! 裸名参数只认局部绑定
    let CountPreds {
        approval_gate,
        approval_pred,
        medium_pred,
        grade_pred,
        edition_pred,
        alive_pred,
        status_pred,
        bookmark_pred,
        search_pred,
        tag_pred,
        sec_sql,
        extra_sql,
    } = preds;
    // 两条 SQL 仅差 alive_pred（hidden_dead = 去存活谓词的计数差值）——
    // 同一模板，插空串即得 all_sql，杜绝手抄漂移。
    let mk_sql = |alive: &str| {
        format!(
            "SELECT count(*) FROM (SELECT 1 FROM torrents t \
             LEFT JOIN users u ON u.id = t.owner_id \
             WHERE {approval_gate} \
             AND ($1::int[] IS NULL OR t.category_id = ANY($1)) \
             AND {medium_pred} AND {grade_pred} AND {edition_pred} \
             AND ($5::bool IS NULL OR t.official_tag = $5) \
             {alive} {approval_pred} {status_pred} \
             {bookmark_pred} {search_pred} \
             {tag_pred} \
             {sec_sql} {extra_sql}{cap}) sub",
            alive = alive,
            cap = if cap > 0 {
                format!(" LIMIT {cap}")
            } else {
                String::new()
            },
        )
    };
    let total: i64 = fetch_count(&mk_sql(alive_pred), args, db)
        .await
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "列表计数查询失败，降级 total=0");
            0
        });
    // hidden_dead（0102 三态可见性补齐）：仅默认视图（alive_pred 非空）
    // 有意义；封顶采样下差值不精确，宁给 0 不给截断假数。
    if alive_pred.is_empty() {
        return (total, 0);
    }
    let all_total: i64 = fetch_count(&mk_sql(""), args, db)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "hidden_dead 计数失败，降级 0");
            0
        });
    let hidden = if cap > 0 && all_total >= cap {
        0
    } else {
        (all_total - total).max(0)
    };
    (total, hidden)
}

/// 计数侧谓词包：与列表侧同一批字符串（计数编号 $7/$8/$9..），由
/// list_noclamp_as 组装后原样传入——两条 SQL 间只差 alive_pred。
pub(super) struct CountPreds {
    pub(super) approval_gate: String,
    pub(super) approval_pred: String,
    pub(super) medium_pred: String,
    pub(super) grade_pred: String,
    pub(super) edition_pred: String,
    pub(super) alive_pred: String,
    pub(super) status_pred: String,
    pub(super) bookmark_pred: String,
    pub(super) search_pred: String,
    pub(super) tag_pred: String,
    pub(super) sec_sql: String,
    pub(super) extra_sql: String,
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
