//! 免钳制列表核心（M02 管理面）：noclamp_as 直查（含全部谓词构造）。
//! 从 torrents/list_noclamp.rs 按域拆出。

use super::list::{esc_like, extra_preds, sort_expr, ExtraSlots};
use super::types::{TorrentFilter, TorrentPage, TorrentRow};
use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;
/// 审计修复（P1）：list_torrents_as 此前把 viewer 转成字符串后丢弃，内部又硬编码
/// viewer_sql="0"，status=seeding/leeching/... 全部按「无人」匹配恒空。现在把
/// viewer 一路传到 SQL 构造（i64 内插无注入面）。
pub async fn list_torrents_noclamp_as(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
    viewer: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.max(1);
    let viewer_sql = viewer.to_string();
    // 匹配模式（旧站 torrents.php 口径）：0/缺省 = AND 模糊；2 = 精确等值（不带通配）
    let exact = filter.search_mode == Some(2);
    let pattern = filter.search.as_deref().map(|s| {
        if exact {
            s.to_string()
        } else {
            format!("%{}%", esc_like(s))
        }
    });
    // 排除词/发布者：与关键字同口径（模糊 + 转义）
    let exclude_pat = filter
        .exclude
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", esc_like(s)));
    let owner_pat = filter
        .owner
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", esc_like(s)));

    // 排序白名单（防注入）；非 id 排序时退化为 OFFSET 无关的「前 N 截断」：
    // 排序键 + id 组成稳定排序，游标仍按 id 翻页（与默认排序一致，简单可靠）。
    // 置顶口径（0063 起；0089 扩展二级置顶）：pos_state 1=一级 2=二级，pos_state_until 到期自动回落，
    // 旧列 sticky 仍被官种联动使用——「任一生效即置顶」，一级 > 二级 > 普通置顶。
    let sticky_expr = "(GREATEST(t.sticky::int, CASE WHEN t.pos_state IN (1, 2) AND (t.pos_state_until IS NULL OR t.pos_state_until > now()) THEN CASE t.pos_state WHEN 1 THEN 2 WHEN 2 THEN 1 ELSE 0 END ELSE 0 END)) DESC";
    // 表头排序（NP colhead 口径）：asc 前缀反转方向；comments = 评论数
    let (key, asc) = match filter.sort.as_deref() {
        Some(s) if s.ends_with("_asc") => (&s[..s.len() - 4], true),
        other => (other.unwrap_or(""), false),
    };
    let order = match key {
        "seeders" => sort_expr(&sticky_expr, "t.seeders", asc),
        "leechers" => sort_expr(&sticky_expr, "t.leechers", asc),
        "size" => sort_expr(&sticky_expr, "t.size", asc),
        "completed" => sort_expr(&sticky_expr, "t.times_completed", asc),
        // 发布时间/名称排序（高级搜索排序扩展；NP「按发布时间/标题」口径）
        "created" => sort_expr(&sticky_expr, "t.created_at", asc),
        "name" => sort_expr(&sticky_expr, "t.name", asc),
        "comments" => sort_expr(
            &sticky_expr,
            "(SELECT count(*) FROM comments c WHERE c.torrent_id = t.id)",
            asc,
        ),
        _ => format!("{sticky_expr}, t.id DESC"),
    };
    // 第八轮 Section 多维筛选：每个维度一个子查询谓词（kind 以 section_kinds 存在性校验 + i64 内插，无注入面）
    // 多维多选（0102）：同维度多值 OR（= ANY），跨维度 AND
    let mut sec_sql = String::new();
    {
        use std::collections::BTreeMap;
        let mut by_kind: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
        for (kind, dict_id) in &filter.sections {
            by_kind.entry(kind).or_default().push(*dict_id);
        }
        for (kind, ids) in by_kind {
            if !crate::admin_p3_http::is_custom_kind(db, kind).await {
                continue;
            }
            let list = ids
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            sec_sql.push_str(&format!(
                " AND t.id IN (SELECT torrent_id FROM torrent_sections WHERE kind = '{kind}' AND dict_id = ANY(ARRAY[{list}]::bigint[]))"
            ));
        }
    }
    // 搜索范围分流（旧站口径）：0=标题+全字段(默认) 1=副标题/简介 3=发布者 4=IMDb
    let esc = if exact { "" } else { " ESCAPE chr(92)" };
    let search_pred = match filter.search_area.unwrap_or(0) {
        1 => format!("AND ($6::text IS NULL OR t.small_descr ILIKE $6{esc} OR t.descr ILIKE $6{esc})"),
        3 => format!("AND ($6::text IS NULL OR u.username ILIKE $6{esc})"),
        4 => format!("AND ($6::text IS NULL OR t.media_info->>'imdb' ILIKE $6{esc} OR t.descr ILIKE $6{esc})"),
        _ => format!("AND ($6::text IS NULL OR t.name ILIKE $6{esc}                OR t.small_descr ILIKE $6{esc}                OR t.descr ILIKE $6{esc}                OR t.id IN (SELECT torrent_id FROM files WHERE path ILIKE $6{esc}))"),
    };

    // 存活三态（0102）：alive 显式给出时覆盖 include_dead（0=全部 1=活种 2=断种）
    let alive_pred = match filter.alive {
        Some(0) => String::new(),
        Some(2) => " AND t.seeders = 0".into(),
        _ => {
            if filter.include_dead {
                String::new()
            } else {
                " AND t.seeders > 0".into()
            }
        }
    };
    // 审核状态（0102）：0=全部 1=通过（默认） 2=被拒（入口已按 see_banned 剥离）
    let approval_pred: String = match filter.approval {
        Some(0) | None => " AND t.approval_status = 1".into(),
        Some(2) => " AND t.approval_status IN (2, 3)".into(),
        Some(1) => " AND t.approval_status = 1".into(),
        Some(_) => " AND t.approval_status = 1".into(),
    };
    // 种子状态（0102，viewer 维度）：需要 snatches 存在性判断（viewer 由调用方注入 SQL 文本，参数化见 bind）
    let status_pred = match filter.status.as_deref() {
        Some("seeding") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.seeding)".to_string(),
        Some("leeching") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.leeching)".to_string(),
        Some("completed") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.completed_at IS NOT NULL)".to_string(),
        Some("incomplete") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.completed_at IS NULL AND (s.uploaded > 0 OR s.downloaded > 0))".to_string(),
        Some("notseeding") => " AND NOT EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.seeding)".to_string(),
        _ => String::new(),
    }
    .replace("{viewer}", &viewer_sql);

    // 高级搜索增强：体积/时间/做种数/排除词/优惠/发布者/仅我发布（列表侧编号 $11..$20）
    // 0118 补齐：下载数/完成数区间 + 匿名发布（列表侧 $21..$25）
    let extra_sql = extra_preds(ExtraSlots {
        size_min: 11,
        size_max: 12,
        date_from: 13,
        date_to: 14,
        min_seeders: 15,
        max_seeders: 16,
        exclude: 17,
        promo: 18,
        owner: 19,
        mine: 20,
        min_leechers: 21,
        max_leechers: 22,
        min_completed: 23,
        max_completed: 24,
        anonymous: 25,
    });
    let sql = format!(
        r#"
        SELECT t.id, t.info_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
               (SELECT p.kind::text FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion,
               (SELECT p.ends_at FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion_ends_at,
               t.media_info->>'rating' AS rating,
               t.media_info->>'poster' AS poster,
               t.created_at
        FROM torrents t
        LEFT JOIN users u ON u.id = t.owner_id
        WHERE (t.approval_status = 1 OR $10::bool)
          AND ($1::int[] IS NULL OR t.category_id = ANY($1))
          AND ($2::int IS NULL OR t.medium_id = $2)
          AND ($3::int IS NULL OR t.grade_id = $3)
          AND ($4::int IS NULL OR t.edition_id = $4)
          AND ($5::bool IS NULL OR t.official_tag = $5)
          {alive_pred}
          {approval_pred}
          {status_pred}
          {search_pred}
          AND ($7::bigint IS NULL OR t.id < $7)
          AND ($9::int IS NULL OR t.id IN (SELECT torrent_id FROM tags WHERE tag_id = $9))
          {sec_sql}
          {extra_sql}
        ORDER BY {order}
        LIMIT $8
        "#
    );
    let rows = sqlx::query_as::<_, TorrentRow>(&sql)
        .bind(filter.category_id.clone())
        .bind(filter.medium_id)
        .bind(filter.grade_id)
        .bind(filter.edition_id)
        .bind(filter.official)
        .bind(&pattern)
        .bind(cursor)
        .bind(limit + 1)
        .bind(filter.tag_id)
        .bind(filter.include_unapproved)
        .bind(filter.size_min)
        .bind(filter.size_max)
        .bind(filter.date_from.as_deref())
        .bind(filter.date_to.as_deref())
        .bind(filter.min_seeders)
        .bind(filter.max_seeders)
        .bind(&exclude_pat)
        .bind(filter.promo.as_deref())
        .bind(&owner_pat)
        .bind(filter.only_mine.then_some(viewer))
        .bind(filter.min_leechers)
        .bind(filter.max_leechers)
        .bind(filter.min_completed)
        .bind(filter.max_completed)
        .bind(filter.anonymous)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 计数与列表同口径（0093 修复）：此前 count 只用「分类/媒介/学段/版本/官种/死种/标题搜索」，
    // 多维筛选、标签、搜索范围（副标题/发布者/IMDb）一律不计入 —— 页面会显示「共 N 个」却零行。
    // 计数同口径：额外谓词在计数侧编号 $9..$18（0118 补齐后 $19..$23）
    let extra_count_sql = extra_preds(ExtraSlots {
        size_min: 9,
        size_max: 10,
        date_from: 11,
        date_to: 12,
        min_seeders: 13,
        max_seeders: 14,
        exclude: 15,
        promo: 16,
        owner: 17,
        mine: 18,
        min_leechers: 19,
        max_leechers: 20,
        min_completed: 21,
        max_completed: 22,
        anonymous: 23,
    });
    let count_sql = format!(
        "SELECT count(*) FROM torrents t LEFT JOIN users u ON u.id = t.owner_id \
         WHERE (t.approval_status = 1 OR $7::bool) \
         AND ($1::int[] IS NULL OR t.category_id = ANY($1)) AND ($2::int IS NULL OR t.medium_id = $2) \
         AND ($3::int IS NULL OR t.grade_id = $3) AND ($4::int IS NULL OR t.edition_id = $4) \
         AND ($5::bool IS NULL OR t.official_tag = $5) {alive_pred} {approval_pred} {status_pred} \
         {search_pred} \
         AND ($8::int IS NULL OR t.id IN (SELECT torrent_id FROM tags WHERE tag_id = $8)) \
         {sec_sql} {extra_count_sql}",
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(filter.category_id.clone())
        .bind(filter.medium_id)
        .bind(filter.grade_id)
        .bind(filter.edition_id)
        .bind(filter.official)
        .bind(&pattern)
        .bind(filter.include_unapproved)
        .bind(filter.tag_id)
        .bind(filter.size_min)
        .bind(filter.size_max)
        .bind(filter.date_from.as_deref())
        .bind(filter.date_to.as_deref())
        .bind(filter.min_seeders)
        .bind(filter.max_seeders)
        .bind(&exclude_pat)
        .bind(filter.promo.as_deref())
        .bind(&owner_pat)
        .bind(filter.only_mine.then_some(viewer))
        .bind(filter.min_leechers)
        .bind(filter.max_leechers)
        .bind(filter.min_completed)
        .bind(filter.max_completed)
        .bind(filter.anonymous)
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let has_more = rows.len() as i64 > limit;
    let items = rows.into_iter().take(limit as usize).collect::<Vec<_>>();
    let next_cursor = has_more
        .then(|| items.last().map(|r| r.id.to_string()).unwrap_or_default());
    Ok(TorrentPage {
        items,
        next_cursor,
        total_estimate: total,
    })
}
