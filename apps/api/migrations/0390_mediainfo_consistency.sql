-- 0390_mediainfo_consistency.sql
-- MediaInfo 一致性比对（评估报告 §1.5，防假 4K/HDR 标题党）
--
-- 现状：MediaInfo 只被**存下来展示**（torrents.media_info->mediainfo），
-- 从没被解析过。于是「标题写 2160p HDR 而实际是 1080p SDR」这种最常见
-- 的内容质量问题，全靠用户举报 + 人工看，滞后且覆盖不了。
--
-- 本迁移加两样：
--   ① torrents 的比对结论列（只存结论，不存全文）：解析摘要 + 是否一致
--      + 差异列表。详情页与审核台据此显示「规格与标题不符」提示，
--      管理组一眼能看到该看哪里，而不必每次手工打开 MediaInfo 对照；
--   ② 巡检视图：一次性列出全站「声明 HDR 但实际 SDR」「声明 4K 但
--      实际 1080p」的种子——这类存量问题靠逐个举报永远清不完。
--
-- 【判定是提示不是闸门】不一致不拦截发布、不自动下架：
--   * 解析器对少见格式会失准，把合法片源误判成标题党，比放过几个
--     标题党更伤用户信任；
--   * 结论可由审核员复核后人工处置，不与解析器绑死。
--   故只在 0389 之后的站上默认「提示」，由审核台决定是否处置。

ALTER TABLE torrents
    ADD COLUMN IF NOT EXISTS mediainfo_check JSONB;

COMMENT ON COLUMN torrents.mediainfo_check IS
    'MediaInfo 一致性比对结论（0390）。JSONB：
     {checked_at, summary:{resolution,video_codec,hdr,bit_depth,audio_codec},
      mismatches:[{field,declared,actual,why}], unverifiable:[...]}
     解析与判据在 domain::mediainfo；**只存结论不存全文**——
     原文可能含站内路径/用户名，进库即泄露。';

-- 巡检视图：标题声明与实际规格不符的种子。
-- 判据写成 SQL 是为了让管理组能直接 ORDER BY 排序导出，
-- 不必先把全站 MediaInfo 拉回应用层。
CREATE OR REPLACE VIEW torrent_mediainfo_mismatch AS
SELECT t.id,
       t.name,
       t.owner_id AS uploader,
       t.approval_status,
       (t.mediainfo_check->'summary'->>'resolution')   AS actual_resolution,
       (t.mediainfo_check->'summary'->>'video_codec')   AS actual_codec,
       (t.mediainfo_check->'summary'->>'hdr')           AS actual_hdr,
       jsonb_array_length(
           COALESCE(t.mediainfo_check->'mismatches', '[]'::jsonb)
       ) AS mismatch_count,
       t.mediainfo_check->'mismatches'                  AS mismatches,
       t.created_at AS added
  FROM torrents t
 WHERE t.mediainfo_check IS NOT NULL
   AND jsonb_array_length(
           COALESCE(t.mediainfo_check->'mismatches', '[]'::jsonb)
       ) > 0;

COMMENT ON VIEW torrent_mediainfo_mismatch IS
    '标题声明与 MediaInfo 实际规格不符的种子（0390 巡检面）。'
    '用于一次性清理存量标题党——逐个举报永远清不完。';

-- 索引：审核台按「不一致」筛选，只索引有结论的行
CREATE INDEX IF NOT EXISTS idx_torrents_mediainfo_mismatch
    ON torrents (created_at DESC)
    WHERE mediainfo_check IS NOT NULL
      AND jsonb_array_length(
              COALESCE(mediainfo_check->'mismatches', '[]'::jsonb)
          ) > 0;

COMMENT ON INDEX idx_torrents_mediainfo_mismatch IS
    '有不一致结论的种子（部分索引）。审核台筛「规格不符」只扫这一小撮，'
    '不对全站 torrents 做 jsonb 解析。';
