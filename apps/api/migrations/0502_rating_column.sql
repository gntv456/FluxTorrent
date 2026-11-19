-- 0502 评分物化列（体检五轮）：torrents.rating numeric(3,1)
--
-- 背景：media_info->>'rating' 全仓零写入者（dev.25 实锤的死键），
-- 评分正源是 work_meta.rating（0399，TMDB/豆瓣 upsert）。dev.25 已把
-- rating_min 谓词改 EXISTS work_meta 救活筛选；本批物化到 torrents 行，
-- 六处死读键一次切换、谓词简化为列比较（可走索引）。
--
-- 维护链：work_meta.rs upsert() 尾部同事务回填（单收口点覆盖
-- TMDB/豆瓣/发种补全/手动重试/管理回填全部链路）。
-- 已知容忍：种子归组（group_id 变更）会使键漂移、物化值短暂陈旧，
-- 至下次 meta upsert/回填自愈——归组是低频站务操作。

ALTER TABLE torrents ADD COLUMN IF NOT EXISTS rating numeric(3,1);

COMMENT ON COLUMN torrents.rating IS
  '作品评分（物化自 work_meta.rating；写入钩子在 publish_http/work_meta.rs upsert）';

-- 存量回填：WK 归一化表达式内联自 works_sql.rs:34-58（迁移无法引用
-- Rust 常量，此处为受控复制——改 WK 时须同步本段）。
UPDATE torrents t SET rating = wm.rating
  FROM work_meta wm
 WHERE wm.key = CASE
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
    END
   AND wm.rating > 0
   AND t.rating IS DISTINCT FROM wm.rating;

-- 筛选索引：rating_min 列化后可走（部分索引只盖有评分行，体积小）
CREATE INDEX IF NOT EXISTS torrents_rating_idx
    ON torrents (rating) WHERE rating IS NOT NULL;
