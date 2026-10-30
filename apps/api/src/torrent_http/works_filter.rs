//! 作品聚合的筛选解析：把 ListQuery 归一成 TorrentFilter，供 works 查询
//! 与列表复用同一套谓词。作品聚合只读已过审种子（资源元数据口径），
//! 因此 approval / include_unapproved / 旧三列筛选在这里显式拒绝，
//! 不再静默忽略。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};
use crate::torrents::TorrentFilter;

use super::query::{
    anonymous_or_err, date_or_err, norm_tags, norm_text, promo_or_err,
    size_or_err, ListQuery,
};

pub(super) struct WorksFilter {
    pub filter: TorrentFilter,
    /// 已转义/加通配的关键字模式（搜索谓词 $6）
    pub pattern: Option<String>,
    pub exclude_pat: Option<String>,
    pub owner_pat: Option<String>,
}

fn fuzz(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", crate::torrents::esc_like(s)))
}

async fn expand_categories(
    db: &PgPool,
    q: &ListQuery,
) -> DomainResult<Option<Vec<i32>>> {
    let mut raw: Vec<&str> =
        q.category_ids.iter().map(String::as_str).collect();
    if let Some(one) = q.category_id_alias.as_deref() {
        raw.push(one);
    }
    let toks: Vec<&str> = raw
        .iter()
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut ids: Vec<i32> = Vec::with_capacity(toks.len());
    for t in toks {
        let v = t.parse::<i32>().map_err(|_| {
            DomainError::Validation(format!("分类参数非法：{t}"))
        })?;
        if !ids.contains(&v) {
            ids.push(v);
        }
    }
    if ids.is_empty() {
        return Ok(None);
    }
    let rows: Vec<i32> = sqlx::query_scalar(
        "WITH RECURSIVE sub AS ( \
         SELECT id FROM categories WHERE id = ANY($1) UNION \
         SELECT c.id FROM categories c JOIN sub s ON c.parent_id = s.id \
         ) SELECT id FROM sub",
    )
    .bind(&ids)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let mut all = ids.clone();
    for r in rows {
        if !all.contains(&r) {
            all.push(r);
        }
    }
    Ok(Some(all))
}

pub(super) async fn parse(
    db: &PgPool,
    query: &str,
    q: &ListQuery,
    q_alias: Option<&str>,
) -> DomainResult<WorksFilter> {
    if let Some(v) = q.approval {
        if v != 0 && v != 1 {
            return Err(DomainError::Validation(
                "作品聚合只展示已过审资源；审核状态筛选请回到种子列表".into(),
            ));
        }
    }
    if q.include_unapproved.unwrap_or(false) {
        return Err(DomainError::Validation(
            "作品聚合只展示已过审资源；include_unapproved 不适用".into(),
        ));
    }
    if q.medium_id.is_some() || q.grade_id.is_some() || q.edition_id.is_some() {
        return Err(DomainError::Validation(
            "作品聚合暂不支持媒介/学段/版本旧列筛选".into(),
        ));
    }
    let category_id = expand_categories(db, q).await?;
    let (tag_ids, tag_all) =
        norm_tags(q.tag_id, &q.tag_ids, q.tag_mode.as_deref());
    let sections = super::sec_params::parse_section_params(db, query).await;
    let search = q_alias
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .or_else(|| q.search.clone());
    let mut filter = TorrentFilter {
        category_id,
        official: q.official,
        search,
        sort: q.sort.clone(),
        tag_ids,
        tag_all,
        sections,
        search_area: q.search_area,
        search_mode: q.search_mode,
        alive: q.alive,
        status: q.status.clone().filter(|s| !s.is_empty()),
        size_min: size_or_err(q.size_min.clone())?,
        size_max: size_or_err(q.size_max.clone())?,
        date_from: date_or_err(q.date_from.clone())?,
        date_to: date_or_err(q.date_to.clone())?,
        min_seeders: q.min_seeders.filter(|v| *v >= 0),
        max_seeders: q.max_seeders.filter(|v| *v >= 0),
        exclude: norm_text(q.exclude.clone()),
        promo: promo_or_err(q.promo.clone())?,
        owner: norm_text(q.owner.clone()),
        only_mine: q.mine.unwrap_or(false),
        min_leechers: q.min_leechers.filter(|v| *v >= 0),
        max_leechers: q.max_leechers.filter(|v| *v >= 0),
        min_completed: q.min_completed.filter(|v| *v >= 0),
        max_completed: q.max_completed.filter(|v| *v >= 0),
        anonymous: anonymous_or_err(q.anonymous)?,
        bookmarked: q.bookmarked.unwrap_or(false),
        rating_min: q
            .rating_min
            .filter(|v| *v > 0.0 && *v <= 10.0)
            .map(|v| (v * 10.0).round() / 10.0),
        ..Default::default()
    };
    super::list_prefs::swap_bounds(&mut filter);
    let exact = filter.search_mode == Some(2);
    let pattern = filter.search.as_deref().map(|s| {
        let escaped = crate::torrents::esc_like(s);
        if exact {
            escaped
        } else {
            format!("%{escaped}%")
        }
    });
    let exclude_pat = fuzz(&filter.exclude);
    let owner_pat = fuzz(&filter.owner);
    Ok(WorksFilter {
        filter,
        pattern,
        exclude_pat,
        owner_pat,
    })
}

/// 作品匹配筛选（$1..$21），与列表侧同源谓词；作品仅过审。
pub(super) fn filter_sql(
    f: &crate::torrents::TorrentFilter,
    sec_sql: &str,
    search_pred: &str,
    viewer: i64,
) -> String {
    use crate::torrents::{
        bookmark_pred, extra_preds, status_pred, tag_pred, ExtraSlots,
    };
    let alive_pred = match f.alive {
        Some(1) => " AND t.seeders > 0",
        Some(2) => " AND t.seeders = 0",
        _ => "",
    };
    let viewer_sql = viewer.to_string();
    let mut out = String::new();
    out.push_str(" AND ($1::int[] IS NULL OR t.category_id = ANY($1))");
    out.push_str(" AND ($2::bool IS NULL OR t.official_tag = $2)");
    out.push_str(alive_pred);
    out.push_str(search_pred);
    out.push_str(&tag_pred(&f.tag_ids, f.tag_all, 20));
    out.push_str(sec_sql);
    out.push_str(&extra_preds(ExtraSlots {
        size_min: 3,
        size_max: 4,
        date_from: 5,
        date_to: 7,
        min_seeders: 8,
        max_seeders: 9,
        exclude: 10,
        promo: 11,
        owner: 12,
        mine: 13,
        min_leechers: 14,
        max_leechers: 15,
        min_completed: 16,
        max_completed: 17,
        anonymous: 18,
        title_like: 21,
        rating_min: 19,
    }));
    out.push_str(&status_pred(f.status.as_deref(), &viewer_sql));
    if f.bookmarked {
        out.push_str(&bookmark_pred(&viewer_sql));
    }
    out
}
