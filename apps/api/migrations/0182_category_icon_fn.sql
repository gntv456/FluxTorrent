-- 0182：分类图标模式表函数化（二审 R10b 配套）。
--
-- 0166 是一次性 UPDATE，只作用于迁移时已存在的分类；站型 apply 重建分类
-- 后图标全部回落首字色块。本迁移把模式匹配提为 SQL 函数 pick_category_icon，
-- pack_core 与后台铺装共用同一口径。

CREATE OR REPLACE FUNCTION pick_category_icon(cat_name text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE
    WHEN cat_name LIKE '%电影%' OR cat_name LIKE '%影院%'
      OR cat_name LIKE '%BluRay%' OR cat_name LIKE '%Remux%' THEN 'film'
    WHEN cat_name LIKE '%电视%' OR cat_name LIKE '%剧%'
      OR cat_name LIKE '%综艺%' THEN 'tv'
    WHEN cat_name LIKE '%音乐%' OR cat_name LIKE '%无损%'
      OR cat_name LIKE '%FLAC%' THEN 'music'
    WHEN cat_name LIKE '%动漫%' OR cat_name LIKE '%动画%' THEN 'anime'
    WHEN cat_name LIKE '%游戏%' THEN 'game'
    WHEN cat_name LIKE '%软件%' OR cat_name LIKE '%工具%' THEN 'app'
    WHEN cat_name LIKE '%电子书%' OR cat_name LIKE '%书%'
      OR cat_name LIKE '%课本%' THEN 'book'
    WHEN cat_name LIKE '%体育%' OR cat_name LIKE '%足球%' OR cat_name LIKE '%篮球%'
      OR cat_name LIKE '%赛事%' OR cat_name LIKE '%集锦%' THEN 'sport'
    WHEN cat_name LIKE '%纪录%' OR cat_name LIKE '%教育影音%' THEN 'doc'
    WHEN cat_name LIKE '%学%' OR cat_name LIKE '%教育%'
      OR cat_name LIKE '%课程%' THEN 'edu'
    ELSE ''
  END
$$;

-- 0166 模式表的补射：0166 漏掉的 sports/software/documentary/anime 命中面
UPDATE categories SET icon_key = pick_category_icon(name)
WHERE icon_key = '' AND pick_category_icon(name) <> '';
