//! 种子列表查询构造（M02）：谓词/排序/游标分页四入口。
//! 从 torrents.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::DomainResult;

use super::list_noclamp_as::list_torrents_noclamp_as;
use super::promo;
use super::types::{ListCursor, TorrentFilter, TorrentPage, MAX_LIMIT};

/// LIKE 通配转义（防用户输入的 `%`/`_` 变成通配符；配合 SQL 侧 `ESCAPE chr(92)`）
pub(super) fn esc_like(s: &str) -> String {
    s.replace('\\', "").replace('%', "\\%").replace('_', "\\_")
}

// 旧 sort_expr 已随批次二删除：排序/游标构造移入 list_noclamp_as（keyset 与 ORDER BY 同源）。

// 促销命中口径已收口到 `super::promo`（唯一来源）：列表 SELECT、计数谓词、RSS、兼容层
// 全部引用它，避免「徽标显示免费但按免费筛不出来」这类三处漂移（2026-09-22 方案 P0-5）。

/// 追加筛选的占位符槽位。列表与计数两条 SQL 的参数编号各自独立，
/// 故谓词文本按槽位号生成（同一份语义，两套编号），避免手改编号串号。
#[derive(Clone, Copy)]
pub(super) struct ExtraSlots {
    pub(super) size_min: u32,
    pub(super) size_max: u32,
    pub(super) date_from: u32,
    pub(super) date_to: u32,
    pub(super) min_seeders: u32,
    pub(super) max_seeders: u32,
    pub(super) exclude: u32,
    pub(super) promo: u32,
    pub(super) owner: u32,
    pub(super) mine: u32,
    pub(super) min_leechers: u32,
    pub(super) max_leechers: u32,
    pub(super) min_completed: u32,
    pub(super) max_completed: u32,
    pub(super) anonymous: u32,
    /// 标题必须命中（0267）：pattern 已由调用方转义/加通配
    pub(super) title_like: u32,
}

/// 高级搜索增强谓词（体积/时间/做种数/排除词/优惠/发布者/仅我发布）。
/// 全部走参数化绑定（`$n IS NULL OR ...`），无注入面；谓词是否生效由绑定的 None/Some 决定。
pub(super) fn extra_preds(s: ExtraSlots) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        " AND (${a}::bigint IS NULL OR t.size >= ${a})",
        a = s.size_min
    ));
    out.push_str(&format!(
        " AND (${a}::bigint IS NULL OR t.size <= ${a})",
        a = s.size_max
    ));
    // 日期按「日历日」口径比较（避免时区把当天数据挤出去）：下界含当日 00:00，上界含当日 23:59:59
    out.push_str(&format!(
        " AND (${a}::date IS NULL OR t.created_at >= ${a}::date)",
        a = s.date_from
    ));
    out.push_str(&format!(
        " AND (${a}::date IS NULL OR t.created_at < (${a}::date + 1))",
        a = s.date_to
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.seeders >= ${a})",
        a = s.min_seeders
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.seeders <= ${a})",
        a = s.max_seeders
    ));
    // 排除词：标题与简介任一命中即排除（pattern 已由调用方转义/加通配）
    out.push_str(&format!(
        " AND (${a}::text IS NULL OR (t.name NOT ILIKE ${a} ESCAPE chr(92) \
           AND COALESCE(t.small_descr, '') NOT ILIKE ${a} ESCAPE chr(92) \
           AND COALESCE(t.descr, '') NOT ILIKE ${a} ESCAPE chr(92)))",
        a = s.exclude
    ));
    // 标题必须命中（0267）：Torznab tvsearch 季集收窄等场景；
    // 只看 t.name，不牵副标题/简介（与上面的排除词语义刻意不同）
    out.push_str(&format!(
        " AND (${a}::text IS NULL OR t.name ILIKE ${a} ESCAPE chr(92))",
        a = s.title_like
    ));
    let p = s.promo;
    let (promo_free, promo_x2, promo_half, promo_any) = (
        promo::exists_clause(Some("'free','x2free'")),
        promo::exists_clause(Some("'x2','x2free','x2half'")),
        promo::exists_clause(Some("'half','x2half'")),
        promo::exists_clause(None),
    );
    // 阶段三筛选粒度：promo 支持多选（逗号串，query.rs norm_promo_multi 归一）。
    // 单值口径不变（free/x2/half/any/none 五档任一）；多值时各档 OR（语义：
    // 「免费或 2x」= 命中任一所选档位）。`$p` 绑定的是归一后的逗号串。
    out.push_str(&format!(
        " AND (${p}::text IS NULL \
           OR (${p} = 'any' AND {promo_any}) \
           OR (${p} = 'none' AND NOT {promo_any}) \
           OR (strpos(${p}, 'free') > 0 AND (${p} LIKE 'free,%' OR ${p} LIKE '%,free' \
               OR ${p} LIKE '%,free,%' OR ${p} = 'free') AND {promo_free}) \
           OR (strpos(${p}, 'x2') > 0 AND (${p} LIKE 'x2,%' OR ${p} LIKE '%,x2' \
               OR ${p} LIKE '%,x2,%' OR ${p} = 'x2') AND {promo_x2}) \
           OR (strpos(${p}, 'half') > 0 AND (${p} LIKE 'half,%' OR ${p} LIKE '%,half' \
               OR ${p} LIKE '%,half,%' OR ${p} = 'half') AND {promo_half}))"
    ));
    out.push_str(&format!(
        " AND (${a}::text IS NULL OR u.username ILIKE ${a} ESCAPE chr(92))",
        a = s.owner
    ));
    out.push_str(&format!(
        " AND (${a}::bigint IS NULL OR t.owner_id = ${a})",
        a = s.mine
    ));
    // 0118 高级搜索补齐：下载数 / 完成数区间（与做种数同口径，含边界）
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.leechers >= ${a})",
        a = s.min_leechers
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.leechers <= ${a})",
        a = s.max_leechers
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.times_completed >= ${a})",
        a = s.min_completed
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.times_completed <= ${a})",
        a = s.max_completed
    ));
    // 匿名发布三态：1=仅匿名 2=仅具名（缺省/0 不筛；口径同 alive）
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR ${a} = 0 OR (${a} = 1 AND t.anonymous) OR (${a} = 2 AND NOT t.anonymous))",
        a = s.anonymous
    ));
    out
}

/// 无视角列表（viewer=0）：不携带 snatches 状态筛选语义的通用入口。
#[allow(dead_code)] // 兼容保留：外部端点已迁移至 list_torrents_noclamp / list_torrents_as
pub async fn list_torrents(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<ListCursor>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    list_torrents_as(db, filter, cursor, limit, 0).await
}

/// viewer 版本（0102 种子状态筛选需要 snatches.user_id 视角；0 = 无人视角 = 状态筛选空转）
pub async fn list_torrents_as(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<ListCursor>,
    limit: i64,
    viewer: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.clamp(1, MAX_LIMIT);
    list_torrents_noclamp_as(db, filter, cursor, limit, viewer).await
}
