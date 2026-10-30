//! 作品聚合 SQL（资源页「作品聚合」视图 + 作品详情）。
//!
//! 背景：影视站/资料库的门面是「一部作品 + 它的多个版本」，而库里只有
//! 种子（`torrents`）。作品层不新建实体（本轮口径：自动聚合）：
//!
//! 1. 有 `group_id` 的种子按组走（0069 聚合组，站务显式归组，最可信）；
//! 2. 其余按**归一化标题**兜底：切掉「｜」后的副题、方括号后缀、年份之后的
//!    发行串（2160p/WEB-DL/x265/S01…），小写 + md5 作为稳定键。
//!
//! 键形如 `g12`（组）/ `n<hash>`（标题），两者都自带前缀，永不相撞。
//! 归一化只在 SQL 里做一次（列表/详情/行查询共用同一个 [`WK`] 表达式），
//! 前端拿到的 key 可直接回拼 `/api/v1/works/{key}`。
//!
//! ⚠️ 已知代价：归一化表达式不可走索引 → 作品聚合是一次全表聚合扫描。
//! 因此它是**独立端点**（`/works`），不塞进 `/torrents`——普通列表的
//! 延迟不受影响。库大到需要时再上物化列（`torrents.work_key` + 索引）。

/// 归一化作品键表达式（对 `t` 别名求值）。
///
/// 正则顺序即优先级：方括号 > 年份 > 发行串；`split_part` 先吃掉中文全角
/// 竖线分隔的副题（「片名｜第几集｜4K」这类命名在本站最常见）。
pub(crate) fn wk_expr() -> &'static str {
    WK
}

/// 作品条目缓存列（work_meta 0399）：作品页零外呼读缓存；缺行给 null 对象，
/// 前端按 `meta.source` 判有无（缺了回落 media_info/descr 口径）。
const META_SEL: &str = "jsonb_build_object(\
   'year', wm.year, 'rating', wm.rating, \
   'runtime', wm.runtime, 'region', wm.region, 'director', wm.director, \
   'cast_list', wm.cast_list, 'genres', wm.genres, 'poster', wm.poster, \
   'synopsis', wm.synopsis, 'source', wm.source) AS meta";
const META_JOIN: &str = "LEFT JOIN work_meta wm ON wm.key = a.wk";
pub(super) const WK: &str = r#"CASE
      WHEN t.group_id IS NOT NULL THEN 'g' || t.group_id::text
      WHEN COALESCE(NULLIF(t.imdb_id, ''),
                    NULLIF(t.media_info->>'imdb_id', ''))
           IS NOT NULL THEN 'i' || COALESCE(NULLIF(t.imdb_id, ''),
                    NULLIF(t.media_info->>'imdb_id', ''))
      WHEN NULLIF(t.media_info->>'tmdb_id', '') IS NOT NULL
           THEN 'm' || (t.media_info->>'tmdb_id')
      WHEN NULLIF(t.media_info->>'douban_id', '') IS NOT NULL
           THEN 'd' || (t.media_info->>'douban_id')
      ELSE 'n' || md5(lower(COALESCE(NULLIF(btrim(
        regexp_replace(
          regexp_replace(
            regexp_replace(
              regexp_replace(
                split_part(split_part(t.name, '｜', 1), '|', 1),
                '^\s*(?:[\[\(（【][^\]\)）】]*[\]\)）】]\s*)+', ''),
              '(\s*[\[\(（【][^\]\)）】]*[\]\)）】])+\s*$', ''),
            '[._\s](19|20)[0-9]{2}([._\s].*)?$', '', 'i'),
          '[._\s](2160p|1080p|1080i|720p|480p|WEB[-.]?DL|WEBRip|'
          'BluRay|Blu-ray|REMUX|HDTV|HDRip|DVDRip|UHD|x26[45]|H\.?26[45]|'
          'HEVC|AVC|S[0-9]{2}(E[0-9]{1,3})?)([._\s].*)?$', '', 'i')),
        ''), btrim(split_part(split_part(t.name, '｜', 1), '|', 1)),
        t.name)))
    END"#;

/// 作品列表（分页）。筛选谓词与列表侧共用槽位：
/// `$1` 分类数组 / `$2` 官种 / `$3-$21` 高级筛选 / `$22` limit / `$23` offset。
/// 筛选先落到 matched（作品键集合），再取该作品的全部过审种子聚合——
/// 作品级筛选不应把同一部片的其它版本从卡里切掉。
pub(super) fn list_sql(
    sort: &str,
    filter: &crate::torrents::TorrentFilter,
    sec_sql: &str,
    search_pred: &str,
    viewer: i64,
) -> String {
    format!(
        r#"
WITH matched AS (
  SELECT DISTINCT {wk} AS wk
    FROM torrents t
    LEFT JOIN users u ON u.id = t.owner_id
   WHERE t.approval_status = 1
     {filters}
),
src AS (
  SELECT t.id, t.name, t.category_id, t.size, t.seeders, t.leechers,
         t.times_completed, t.created_at, t.small_descr, t.descr,
         t.media_info, t.group_id, t.imdb_id, {wk} AS wk
    FROM torrents t
   WHERE t.approval_status = 1
     AND {wk} IN (SELECT wk FROM matched)
),
agg AS (
  SELECT wk,
         count(*)::bigint          AS torrents,
         sum(seeders)::bigint       AS seeders,
         sum(leechers)::bigint      AS leechers,
         sum(size)::bigint          AS size,
         sum(times_completed)::bigint AS completed,
         max(created_at)            AS latest_at,
         max(id)                    AS top_id
    FROM src GROUP BY wk
),
rep AS (
  SELECT DISTINCT ON (wk) wk, id, name, small_descr, descr, category_id,
         media_info->>'poster' AS poster,
         media_info->>'rating' AS rating,
         COALESCE(NULLIF(imdb_id, ''), media_info->>'imdb_id') AS imdb_id,
         media_info->>'tmdb_id' AS tmdb_id,
         media_info->>'tmdb_url' AS tmdb_url,
         media_info->>'douban_id' AS douban_id
    FROM src
   ORDER BY wk, (media_info->>'poster') IS NOT NULL DESC,
            created_at DESC, id DESC
)
SELECT a.wk AS key, r.name, r.poster, r.rating, r.category_id,
       r.small_descr AS synopsis, r.descr AS detail,
       r.imdb_id, r.tmdb_id, r.tmdb_url, r.douban_id,
       a.torrents, a.seeders, a.leechers,
       a.size, a.latest_at, a.top_id,
       (substring(r.name from '((19|20)[0-9]{{2}})'))::int AS year,
       (SELECT COALESCE(json_agg(json_build_object(
            'id', d.id, 'name', d.name, 'kind', d.kind,
            'bg_color', d.bg_color, 'color', d.color)
            ORDER BY d.sort DESC, d.id), '[]'::json)
           FROM tags tg JOIN tag_dict d ON d.id = tg.tag_id
          WHERE tg.torrent_id = a.top_id) AS tags,
       {meta_sel}
  FROM agg a JOIN rep r ON r.wk = a.wk
  {meta_join}
 ORDER BY {sort}
 LIMIT $22 OFFSET $23"#,
        wk = WK,
        filters = super::works_filter::filter_sql(
            filter,
            sec_sql,
            search_pred,
            viewer
        ),
        meta_sel = META_SEL,
        meta_join = META_JOIN,
        sort = sort
    )
}

/// 作品计数（与列表同口径：相同筛选后的作品键数）。
pub(super) fn count_sql(
    filter: &crate::torrents::TorrentFilter,
    sec_sql: &str,
    search_pred: &str,
    viewer: i64,
) -> String {
    format!(
        r#"
WITH matched AS (
  SELECT DISTINCT {wk} AS wk
    FROM torrents t
    LEFT JOIN users u ON u.id = t.owner_id
   WHERE t.approval_status = 1
     {filters}
)
SELECT count(*)::bigint FROM matched"#,
        wk = WK,
        filters = super::works_filter::filter_sql(
            filter,
            sec_sql,
            search_pred,
            viewer
        )
    )
}

/// 单个作品（`$1` = 作品键）——详情页头部用，字段与列表同形。
pub(super) fn one_sql() -> String {
    format!(
        r#"
WITH src AS (
  SELECT t.id, t.name, t.category_id, t.size, t.seeders, t.leechers,
         t.created_at, t.small_descr, t.descr, t.media_info, t.group_id,
         t.imdb_id, {wk} AS wk
    FROM torrents t
   WHERE t.approval_status = 1
     AND {wk} = $1
),
agg AS (
  SELECT wk, count(*)::bigint AS torrents, sum(seeders)::bigint AS seeders,
         sum(leechers)::bigint AS leechers, sum(size)::bigint AS size,
         max(created_at) AS latest_at, max(id) AS top_id
    FROM src GROUP BY wk
),
rep AS (
  SELECT DISTINCT ON (wk) wk, id, name, small_descr, descr, category_id,
         media_info->>'poster' AS poster,
         media_info->>'rating' AS rating,
         COALESCE(NULLIF(imdb_id, ''), media_info->>'imdb_id') AS imdb_id,
         media_info->>'tmdb_id' AS tmdb_id,
         media_info->>'tmdb_url' AS tmdb_url,
         media_info->>'douban_id' AS douban_id
    FROM src
   ORDER BY wk, (media_info->>'poster') IS NOT NULL DESC,
            created_at DESC, id DESC
)
SELECT a.wk AS key, r.name, r.poster, r.rating, r.category_id,
       r.small_descr AS synopsis, r.descr AS detail,
       r.imdb_id, r.tmdb_id, r.tmdb_url, r.douban_id,
       a.torrents, a.seeders, a.leechers,
       a.size, a.latest_at, a.top_id,
       (substring(r.name from '((19|20)[0-9]{{2}})'))::int AS year,
       (SELECT COALESCE(json_agg(json_build_object(
            'id', d.id, 'name', d.name, 'kind', d.kind,
            'bg_color', d.bg_color, 'color', d.color)
            ORDER BY d.sort DESC, d.id), '[]'::json)
           FROM tags tg JOIN tag_dict d ON d.id = tg.tag_id
          WHERE tg.torrent_id = a.top_id) AS tags,
       {meta_sel}
  FROM agg a JOIN rep r ON r.wk = a.wk
  {meta_join}"#,
        wk = WK,
        meta_sel = META_SEL,
        meta_join = META_JOIN
    )
}

/// 相关推荐（`$1` 分类 / `$2` 排除的作品键）：同分类热门，按做种倒序。
/// 返回仍是作品粒度（先按作品聚合再取前 N），避免推荐位里出现同一片的
/// 三个版本占满三格。
pub(super) fn related_sql() -> String {
    format!(
        r#"
WITH src AS (
  SELECT t.id, t.name, t.category_id, t.size, t.seeders, t.leechers,
         t.created_at, t.media_info,
         {wk} AS wk
    FROM torrents t
   WHERE t.approval_status = 1
     AND t.category_id = $1
),
agg AS (
  SELECT wk, count(*)::bigint AS torrents, sum(seeders)::bigint AS seeders,
         sum(leechers)::bigint AS leechers, sum(size)::bigint AS size,
         max(created_at) AS latest_at, max(id) AS top_id
    FROM src GROUP BY wk
),
rep AS (
  SELECT DISTINCT ON (wk) wk, name, category_id,
         media_info->>'poster' AS poster
    FROM src ORDER BY wk, created_at DESC, id DESC
)
SELECT a.wk AS key, r.name, r.poster, r.category_id, a.torrents,
       a.seeders, a.leechers, a.size, a.latest_at, a.top_id,
       (substring(r.name from '((19|20)[0-9]{{2}})'))::int AS year,
       NULL::text AS rating, NULL::text AS synopsis,
       NULL::text AS detail, NULL::text AS imdb_id,
       NULL::text AS tmdb_id, NULL::text AS tmdb_url,
       NULL::text AS douban_id,
       '[]'::json AS tags,
       {meta_sel}
  FROM agg a JOIN rep r ON r.wk = a.wk
  {meta_join}
 WHERE a.wk <> $2
 ORDER BY a.seeders DESC, a.latest_at DESC
 LIMIT 6"#,
        wk = WK,
        meta_sel = META_SEL,
        meta_join = META_JOIN
    )
}

/// 归一化标题表达式（0424 P1-2 抽出）：种名去「｜之后/前导标签」的纯片名——
/// 作品订阅用它与 WK 的 n-键同一段前导括号归一，两侧不各写一份。
pub(super) fn title_expr() -> &'static str {
    r#"COALESCE(NULLIF(regexp_replace(
             split_part(split_part(t.name, '｜', 1), '|', 1),
             '^\s*(?:[\[\(（【][^\]\)）】]*[\]\)）】]\s*)+', ''), ''),
             t.name)"#
}
