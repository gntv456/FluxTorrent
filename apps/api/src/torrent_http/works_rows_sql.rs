//! 作品聚合的「种子行」查询（作品卡下挂的种子 + 批量取每部前 8 颗）。
//! 从 works_sql.rs 按层拆出（300 行门禁）：那边是作品层（聚合/缓存），
//! WK 表达式也从那里引（对 `t` 别名求值）。
//! 这里是种子行层——列集合与 `/torrents` 列表行同形，前端复用同一套渲染。

use super::works_sql::WK;

/// 作品下的种子行（`$1` = 作品键）。列与 `/torrents` 列表同形，
/// 前端直接复用同一套行渲染（chips / 数字列 / 操作列）。
pub(super) fn rows_sql(promo_lateral: &str) -> String {
    format!(
        r#"
SELECT t.id, t.info_hash, t.pieces_hash, t.name, t.small_descr,
       t.category_id, t.medium_id, t.grade_id, t.edition_id, t.size,
       t.seeders, t.leechers, t.times_completed,
       (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
       t.official_tag, t.anonymous, t.approval_status, t.sticky,
       CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
       pr.promotion, pr.promotion_ends_at,
       t.media_info->>'rating' AS rating,
       t.media_info->>'poster' AS poster,
       t.imdb_id, t.created_at,
       (CASE WHEN jsonb_typeof(t.screenshots) = 'array' THEN
          (SELECT jsonb_agg(v ORDER BY ord) FROM (
             SELECT v, ord FROM jsonb_array_elements_text(t.screenshots)
               WITH ORDINALITY AS e(v, ord) ORDER BY ord LIMIT 4
           ) s) END) AS screenshots,
       (SELECT COALESCE(json_agg(json_build_object(
            'id', d.id, 'name', d.name, 'kind', d.kind,
            'bg_color', d.bg_color, 'color', d.color)
            ORDER BY d.sort DESC, d.id), '[]'::json)
          FROM tags tg JOIN tag_dict d ON d.id = tg.tag_id
         WHERE tg.torrent_id = t.id) AS tags,
       (SELECT COALESCE(json_agg(
            COALESCE(x.name, ts.value #>> '{{}}')
            ORDER BY k.sort, k.kind, ts.ordinal), '[]'::json)
          FROM torrent_sections ts
          LEFT JOIN section_dict x ON x.id = ts.dict_id
          LEFT JOIN section_kinds k ON k.kind = ts.kind
         WHERE ts.torrent_id = t.id) AS sec_names,
       GREATEST(t.sticky::int, CASE WHEN t.pos_state IN (1, 2)
         AND (t.pos_state_until IS NULL OR t.pos_state_until > now())
         THEN CASE t.pos_state WHEN 1 THEN 2 WHEN 2 THEN 1 ELSE 0 END
         ELSE 0 END) AS sticky_rank
  FROM torrents t
  LEFT JOIN users u ON u.id = t.owner_id
  {promo}
 WHERE t.approval_status = 1 AND {wk} = $1
 ORDER BY sticky_rank DESC, t.created_at DESC, t.id DESC"#,
        promo = promo_lateral,
        wk = WK
    )
}

/// 批量取种子行（`$1` = 作品键数组）：作品聚合视图一页 24 部作品，
/// 每部给前 8 个种子——单条 SQL 用窗口函数截断，避免 N+1。
/// 返回行带 `work_key` 列（前端按 key 归堆），列集合与 [`rows_sql`] 同形。
pub(super) fn batch_sql(promo_lateral: &str) -> String {
    format!(
        r#"
WITH src AS (
  SELECT t.id, t.created_at, {wk} AS wk
    FROM torrents t
   WHERE t.approval_status = 1 AND {wk} = ANY($1)
),
pick AS (
  SELECT id, wk, row_number() OVER (
           PARTITION BY wk ORDER BY created_at DESC, id DESC) AS rn
    FROM src
)
SELECT {wk} AS work_key,
       t.id, t.info_hash, t.pieces_hash, t.name, t.small_descr,
       t.category_id, t.medium_id, t.grade_id, t.edition_id, t.size,
       t.seeders, t.leechers, t.times_completed,
       (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
       t.official_tag, t.anonymous, t.approval_status, t.sticky,
       CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
       pr.promotion, pr.promotion_ends_at,
       t.media_info->>'rating' AS rating,
       t.media_info->>'poster' AS poster,
       t.imdb_id, t.created_at,
       (CASE WHEN jsonb_typeof(t.screenshots) = 'array' THEN
          (SELECT jsonb_agg(v ORDER BY ord) FROM (
             SELECT v, ord FROM jsonb_array_elements_text(t.screenshots)
               WITH ORDINALITY AS e(v, ord) ORDER BY ord LIMIT 4
           ) s) END) AS screenshots,
       (SELECT COALESCE(json_agg(json_build_object(
            'id', d.id, 'name', d.name, 'kind', d.kind,
            'bg_color', d.bg_color, 'color', d.color)
            ORDER BY d.sort DESC, d.id), '[]'::json)
          FROM tags tg JOIN tag_dict d ON d.id = tg.tag_id
         WHERE tg.torrent_id = t.id) AS tags,
       (SELECT COALESCE(json_agg(
            COALESCE(x.name, ts.value #>> '{{}}')
            ORDER BY k.sort, k.kind, ts.ordinal), '[]'::json)
          FROM torrent_sections ts
          LEFT JOIN section_dict x ON x.id = ts.dict_id
          LEFT JOIN section_kinds k ON k.kind = ts.kind
         WHERE ts.torrent_id = t.id) AS sec_names,
       0 AS sticky_rank
  FROM pick p
  JOIN torrents t ON t.id = p.id
  LEFT JOIN users u ON u.id = t.owner_id
  {promo}
 WHERE p.rn <= 8
 ORDER BY t.created_at DESC, t.id DESC"#,
        promo = promo_lateral,
        wk = WK
    )
}
