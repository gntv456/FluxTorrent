//! 种子列表查询构造（M02）：谓词/排序/游标分页四入口。
//! 从 torrents.rs 按域拆出。

use sqlx::PgPool;

use crate::errors::DomainResult;

use super::list_noclamp_as::list_torrents_noclamp_as;
use super::types::{TorrentFilter, TorrentPage, MAX_LIMIT};

/// LIKE 通配转义（防用户输入的 `%`/`_` 变成通配符；配合 SQL 侧 `ESCAPE chr(92)`）
pub(super) fn esc_like(s: &str) -> String {
    s.replace('\\', "").replace('%', "\\%").replace('_', "\\_")
}

pub(super) fn sort_expr(sticky_expr: &str, col: &str, asc: bool) -> String {
    let dir = if asc { "ASC" } else { "DESC" };
    format!("{sticky_expr}, {col} {dir}, t.id {dir}")
}

/// 优惠（promotions）命中的统一匹配式：种子级直挂 或 全局/官种/非官种/分类作用域。
/// 与列表 SELECT 中 promotion 子查询同口径，保证「筛选出的免费种」与徽标显示一致。
const PROMO_MATCH: &str = "p.starts_at <= now() AND p.ends_at > now() AND (\
     p.torrent_id = t.id OR (p.torrent_id IS NULL AND (\
       p.scope = 'global' \
       OR (p.scope = 'official' AND t.official_tag) \
       OR (p.scope = 'non_official' AND NOT t.official_tag) \
       OR (p.scope = 'category' AND t.category_id = p.category_id))))";

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
    let p = s.promo;
    out.push_str(&format!(
        " AND (${p}::text IS NULL \
           OR (${p} = 'free' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH} AND p.kind::text IN ('free','x2free'))) \
           OR (${p} = 'x2' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH} AND p.kind::text IN ('x2','x2free','x2half'))) \
           OR (${p} = 'half' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH} AND p.kind::text IN ('half','x2half'))) \
           OR (${p} = 'any' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH})) \
           OR (${p} = 'none' AND NOT EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH})))"
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
    cursor: Option<i64>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    list_torrents_as(db, filter, cursor, limit, 0).await
}

/// viewer 版本（0102 种子状态筛选需要 snatches.user_id 视角；0 = 无人视角 = 状态筛选空转）
pub async fn list_torrents_as(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
    viewer: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.clamp(1, MAX_LIMIT);
    list_torrents_noclamp_as(db, filter, cursor, limit, viewer).await
}
