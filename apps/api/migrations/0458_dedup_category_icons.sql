-- 0458: 解撞三组分类图标——短剧/有声书/综艺与电视剧/音乐同脸。
--
-- 现状（站长 2026-11-03 巡库发现）：icon_key「语义撞车」三处——
--   短剧 general_shortdrama = tv        （与电视剧 general_tv 同图标）
--   有声书 general_audiobook / ebook_audio = music（与音乐 general_music 同图标）
--   综艺 general_variety / movie_variety = tv   （与电视剧同图标，0317 老口径）
-- 前端 CatBlock 按图标渲染类型色块（torrent-table-badges.tsx），同图标 =
-- 浏览页两类不可分。同批已在前端 icons.tsx 注册三个专属图标：
--   shortdrama（竖屏+播放键）/ audiobook（书+声波）/ variety（星+横幅）。
--
-- 四层落点（缺一层就是「同一件事两处口径」的半套病）：
--   ① 存量 categories 行改键——只认「旧撞车值」为改写前置（站长手工改成
--      别的键的不动；改成空串的也不动——那是站长显式选择首字色块）；
--   ② 站型包载荷（site_type_packs.categories）逐键改写——不整段重写，
--      站长在后台改过包的其它字段不受影响（0457 同款 jsonb 手术）；
--   ③ 重建防回归：pack_core 的 COALESCE 链对非空 icon_key 是
--      「现有非空优先」，改过键的行重放不会被包载荷的旧值顶回去；
--      但 pick_category_icon（0182）猜名函数会把「短剧」猜成 tv、
--      「有声书」猜成 music、「综艺」猜成 tv——升级函数堵住新分类名
--      再猜回撞车键（IMMUTABLE 函数改的是猜测分支，不改既有命中，
--      台账无 checksum 锁，CREATE OR REPLACE 直接上）；
--   ④ 同病顺手收口（与「综艺≠电视剧」同一判据）：音乐/无损包的
--      Live/演唱会、体育包的集锦——都是「演出/节目」不是剧集，也撞 tv。
--      MV/music_mv 保持 tv：短视频内容形态与竖屏剧集同族，且不在站长
--      点名的三组里，不扩权。
--
-- 幂等：①按「现值=旧撞车键」谓词收敛到一次；②③幂等；重放等值。

BEGIN;

-- ============ ① 存量行：撞车键 → 专属键 ============
UPDATE categories SET icon_key = 'shortdrama'
 WHERE key = 'general_shortdrama' AND icon_key = 'tv';
UPDATE categories SET icon_key = 'audiobook'
 WHERE key IN ('general_audiobook', 'ebook_audio') AND icon_key = 'music';
UPDATE categories SET icon_key = 'variety'
 WHERE key IN ('general_variety', 'movie_variety') AND icon_key = 'tv';
UPDATE categories SET icon_key = 'variety'
 WHERE key IN ('music_live', 'lossless_live') AND icon_key = 'tv';
UPDATE categories SET icon_key = 'variety'
 WHERE key = 'sport_highlights' AND icon_key = 'tv';

-- ============ ② 包载荷：逐键改写（不整段重写，站长改过的包不动其它字段）============
-- general：短剧/有声书/综艺
UPDATE site_type_packs
   SET categories = (
         SELECT jsonb_agg(
                  CASE
                    WHEN c->>'key' = 'general_shortdrama'
                      THEN jsonb_set(c, '{icon_key}', '"shortdrama"')
                    WHEN c->>'key' = 'general_audiobook'
                      THEN jsonb_set(c, '{icon_key}', '"audiobook"')
                    WHEN c->>'key' = 'general_variety'
                      THEN jsonb_set(c, '{icon_key}', '"variety"')
                    ELSE c
                  END
                  ORDER BY (c->>'sort')::int)
           FROM jsonb_array_elements(categories) AS c)
 WHERE code = 'general';

-- ebook：有声书
UPDATE site_type_packs
   SET categories = (
         SELECT jsonb_agg(
                  CASE
                    WHEN c->>'key' = 'ebook_audio'
                      THEN jsonb_set(c, '{icon_key}', '"audiobook"')
                    ELSE c
                  END
                  ORDER BY (c->>'sort')::int)
           FROM jsonb_array_elements(categories) AS c)
 WHERE code = 'ebook';

-- movie：综艺
UPDATE site_type_packs
   SET categories = (
         SELECT jsonb_agg(
                  CASE
                    WHEN c->>'key' = 'movie_variety'
                      THEN jsonb_set(c, '{icon_key}', '"variety"')
                    ELSE c
                  END
                  ORDER BY (c->>'sort')::int)
           FROM jsonb_array_elements(categories) AS c)
 WHERE code = 'movie';

-- music / lossless / sports：Live / 演唱会 / 集锦
UPDATE site_type_packs
   SET categories = (
         SELECT jsonb_agg(
                  CASE
                    WHEN c->>'key' IN ('music_live', 'lossless_live',
                                       'sport_highlights')
                      THEN jsonb_set(c, '{icon_key}', '"variety"')
                    ELSE c
                  END
                  ORDER BY (c->>'sort')::int)
           FROM jsonb_array_elements(categories) AS c)
 WHERE code IN ('music', 'lossless', 'sports');

-- ============ ③ 猜名函数升级（0182 的分支收紧）============
-- 顺序敏感：短剧/有声书/综艺必须在「剧/书/电视」宽泛分支**之前**命中。
-- 只加新分支+挪综艺，既有分类名（电视剧/音乐/无损…）的命中结果不变。
CREATE OR REPLACE FUNCTION pick_category_icon(cat_name text) RETURNS text
LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE
    WHEN cat_name LIKE '%短剧%' THEN 'shortdrama'
    WHEN cat_name LIKE '%有声书%' OR cat_name LIKE '%广播剧%' THEN 'audiobook'
    WHEN cat_name LIKE '%综艺%' THEN 'variety'
    WHEN cat_name LIKE '%演唱会%' OR cat_name LIKE '%Live%'
      OR cat_name LIKE '%集锦%' THEN 'variety'
    WHEN cat_name LIKE '%电影%' OR cat_name LIKE '%影院%'
      OR cat_name LIKE '%BluRay%' OR cat_name LIKE '%Remux%' THEN 'film'
    WHEN cat_name LIKE '%电视%' OR cat_name LIKE '%剧%'
      OR cat_name LIKE '%MV%' THEN 'tv'
    WHEN cat_name LIKE '%音乐%' OR cat_name LIKE '%无损%'
      OR cat_name LIKE '%FLAC%' THEN 'music'
    WHEN cat_name LIKE '%动漫%' OR cat_name LIKE '%动画%' THEN 'anime'
    WHEN cat_name LIKE '%游戏%' THEN 'game'
    WHEN cat_name LIKE '%软件%' OR cat_name LIKE '%工具%' THEN 'app'
    WHEN cat_name LIKE '%电子书%' OR cat_name LIKE '%书%'
      OR cat_name LIKE '%课本%' THEN 'book'
    WHEN cat_name LIKE '%体育%' OR cat_name LIKE '%足球%' OR cat_name LIKE '%篮球%'
      OR cat_name LIKE '%赛事%' THEN 'sport'
    WHEN cat_name LIKE '%纪录%' OR cat_name LIKE '%教育影音%' THEN 'doc'
    WHEN cat_name LIKE '%学%' OR cat_name LIKE '%教育%'
      OR cat_name LIKE '%课程%' THEN 'edu'
    ELSE ''
  END
$$;

COMMIT;
