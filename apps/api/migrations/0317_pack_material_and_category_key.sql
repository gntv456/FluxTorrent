-- 0317_pack_material_and_category_key.sql
-- 站型成熟度批（_doc/站型成熟度对标与补齐方案-2026-10-08.md 批次 0 + 批次 1 前半）：
--
-- ① H1 前置：categories 增加 `key` 列（跨包稳定身份）。分类匹配从「同 id 覆盖」
--    改为「同 key 覆盖」，id 退化为内部主键——切站型不再把老分类原地改名、
--    偷换在用种子的分类语义。回填规则：优先采用「当前站型包声明的 key」，
--    否则按 slug（c1..cN）生成，保证存量行立即有身份。
-- ② H6 素材：11 个预置包 categories 载荷补 key/sort/icon_key/bg_color
--    （能力早就在——apply 会写这些列，只是没有任何包声明过）。
-- ③ 词表补齐：6 个挂 team 维度却空词表的包补出厂项；general/sports/software
--    补标签词表；game/ebook/music/lossless 顺带按成熟站口径补齐。
-- ④ H9 元数据源：music/lossless 按音频口径配 musicbrainz（原 douban+mediainfo）；
--    game/software/sports/ebook/education/general 补该域合理源。
-- ⑤ H9 死声明：losslessPlayer 从包里删掉（不在 modules 注册表、零消费者）。
--
-- 全部幂等：key 回填 WHERE key IS NULL；包载荷 UPDATE 逐包固定值重放等值；
-- 词表/标签/源都是「先剔除同名再追加」或整段替换。

BEGIN;

-- ============================================================
-- ① categories.key（H1）
-- ============================================================
ALTER TABLE categories ADD COLUMN IF NOT EXISTS key TEXT;
-- 唯一约束用约束表达式形式而不是部分索引：ON CONFLICT (key) 需要能推断出
-- 推断索引（partial unique index 带 WHERE 时 ON CONFLICT 指定不了谓词），
-- 而 NULLIF($x,'') 传空串必须走「NULL 不冲突」语义 ⇒ 唯一表达式索引。
CREATE UNIQUE INDEX IF NOT EXISTS idx_categories_key
    ON categories ((NULLIF(key, '')));

-- 回填：当前站型包声明了 key 的行优先采用声明值；其余按 c<id> 兜底。
UPDATE categories c SET key = p.cat->>'key'
FROM site_type_packs pk
JOIN site_settings s ON s.name = 'site_type' AND s.value = pk.code,
     jsonb_array_elements(pk.categories) AS p(cat)
WHERE (p.cat->>'id')::int = c.id
  AND p.cat ? 'key'
  AND c.key IS NULL;

UPDATE categories SET key = 'c' || id::text WHERE key IS NULL;

-- ============================================================
-- ② 11 包 categories 载荷补 key/sort/icon_key/bg_color（H6）
--    key 规则：<packcode>_<语义 slug>，全局唯一不重叠 ⇒ apply 按 key 匹配时
--    不同包的分类互不覆盖，只会追加新行；切回旧包时按 key 找回原行。
--    bg_color 取 Aurora 色板的低饱和段（列表色块既有口径）。
-- ============================================================

-- general（综合站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"general_movie","name":"电影","sort":10,"icon_key":"film","bg_color":"#3b82f6"},
 {"id":2,"key":"general_tv","name":"电视剧","sort":20,"icon_key":"tv","bg_color":"#8b5cf6"},
 {"id":3,"key":"general_music","name":"音乐","sort":30,"icon_key":"music","bg_color":"#ec4899"},
 {"id":4,"key":"general_game","name":"游戏","sort":40,"icon_key":"game","bg_color":"#22c55e"},
 {"id":5,"key":"general_software","name":"软件","sort":50,"icon_key":"app","bg_color":"#14b8a6"},
 {"id":6,"key":"general_anime","name":"动漫","sort":60,"icon_key":"anime","bg_color":"#f97316"},
 {"id":7,"key":"general_doc","name":"纪录片","sort":70,"icon_key":"doc","bg_color":"#0ea5e9"},
 {"id":8,"key":"general_ebook","name":"电子书","sort":80,"icon_key":"book","bg_color":"#a3a615"},
 {"id":9,"key":"general_sports","name":"体育","sort":90,"icon_key":"sport","bg_color":"#eab308"},
 {"id":10,"key":"general_variety","name":"综艺","sort":100,"icon_key":"tv","bg_color":"#d946ef"}
]'::jsonb WHERE code='general';

-- movie（影视站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"movie_bd","name":"电影/BluRay","sort":10,"icon_key":"film","bg_color":"#3b82f6"},
 {"id":2,"key":"movie_remux","name":"电影/Remux","sort":20,"icon_key":"film","bg_color":"#2563eb"},
 {"id":3,"key":"movie_webdl","name":"电影/WEB-DL","sort":30,"icon_key":"film","bg_color":"#1d4ed8"},
 {"id":4,"key":"movie_x264","name":"电影/x264","sort":40,"icon_key":"film","bg_color":"#60a5fa"},
 {"id":5,"key":"movie_tv","name":"电视剧","sort":50,"icon_key":"tv","bg_color":"#8b5cf6"},
 {"id":6,"key":"movie_variety","name":"综艺","sort":60,"icon_key":"tv","bg_color":"#d946ef"},
 {"id":7,"key":"movie_doc","name":"纪录片","sort":70,"icon_key":"doc","bg_color":"#0ea5e9"}
]'::jsonb WHERE code='movie';

-- music（音乐站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"music_cn","name":"华语音乐","sort":10,"icon_key":"music","bg_color":"#ec4899"},
 {"id":2,"key":"music_west","name":"欧美音乐","sort":20,"icon_key":"music","bg_color":"#f472b6"},
 {"id":3,"key":"music_jk","name":"日韩音乐","sort":30,"icon_key":"music","bg_color":"#db2777"},
 {"id":4,"key":"music_single","name":"单曲","sort":40,"icon_key":"music","bg_color":"#fb7185"},
 {"id":5,"key":"music_mv","name":"MV","sort":50,"icon_key":"tv","bg_color":"#d946ef"},
 {"id":6,"key":"music_live","name":"演唱会","sort":60,"icon_key":"tv","bg_color":"#c026d3"},
 {"id":7,"key":"music_lossless","name":"无损","sort":70,"icon_key":"music","bg_color":"#be185d"}
]'::jsonb WHERE code='music';

-- lossless（无损音乐站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"lossless_album","name":"无损专辑","sort":10,"icon_key":"music","bg_color":"#be185d"},
 {"id":2,"key":"lossless_hires","name":"Hi-Res","sort":20,"icon_key":"music","bg_color":"#a21caf"},
 {"id":3,"key":"lossless_vinyl","name":"黑胶转制","sort":30,"icon_key":"music","bg_color":"#86198f"},
 {"id":4,"key":"lossless_sacd","name":"SACD","sort":40,"icon_key":"music","bg_color":"#701a75"},
 {"id":5,"key":"lossless_sample","name":"采样","sort":50,"icon_key":"music","bg_color":"#9d174d"},
 {"id":6,"key":"lossless_single","name":"单曲","sort":60,"icon_key":"music","bg_color":"#fb7185"},
 {"id":7,"key":"lossless_live","name":"Live","sort":70,"icon_key":"tv","bg_color":"#c026d3"}
]'::jsonb WHERE code='lossless';

-- anime（动漫站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"anime_tv","name":"动画","sort":10,"icon_key":"anime","bg_color":"#f97316"},
 {"id":2,"key":"anime_season","name":"季度番剧","sort":20,"icon_key":"anime","bg_color":"#ea580c"},
 {"id":3,"key":"anime_movie","name":"剧场版","sort":30,"icon_key":"film","bg_color":"#fb923c"},
 {"id":4,"key":"anime_manga","name":"漫画","sort":40,"icon_key":"book","bg_color":"#a3a615"},
 {"id":5,"key":"anime_doujin","name":"同人","sort":50,"icon_key":"anime","bg_color":"#fdba74"},
 {"id":6,"key":"anime_music","name":"音乐","sort":60,"icon_key":"music","bg_color":"#ec4899"},
 {"id":7,"key":"anime_goods","name":"周边","sort":70,"icon_key":"app","bg_color":"#f59e0b"}
]'::jsonb WHERE code='anime';

-- ebook（电子书站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"ebook_lit","name":"文学","sort":10,"icon_key":"book","bg_color":"#a3a615"},
 {"id":2,"key":"ebook_social","name":"社科","sort":20,"icon_key":"book","bg_color":"#84cc16"},
 {"id":3,"key":"ebook_tech","name":"科技","sort":30,"icon_key":"book","bg_color":"#65a30d"},
 {"id":4,"key":"ebook_mag","name":"杂志","sort":40,"icon_key":"book","bg_color":"#4d7c0f"},
 {"id":5,"key":"ebook_comic","name":"漫画","sort":50,"icon_key":"anime","bg_color":"#f97316"},
 {"id":6,"key":"ebook_audio","name":"有声书","sort":60,"icon_key":"music","bg_color":"#ec4899"},
 {"id":7,"key":"ebook_guide","name":"教辅","sort":70,"icon_key":"edu","bg_color":"#16a34a"}
]'::jsonb WHERE code='ebook';

-- education（教育站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"edu_preschool","name":"学前教育","sort":10,"icon_key":"edu","bg_color":"#16a34a"},
 {"id":2,"key":"edu_primary","name":"小学","sort":20,"icon_key":"edu","bg_color":"#22c55e"},
 {"id":3,"key":"edu_junior","name":"初中","sort":30,"icon_key":"edu","bg_color":"#15803d"},
 {"id":4,"key":"edu_vocational","name":"职高","sort":40,"icon_key":"edu","bg_color":"#4ade80"},
 {"id":5,"key":"edu_senior","name":"高中","sort":50,"icon_key":"edu","bg_color":"#166534"},
 {"id":6,"key":"edu_media","name":"教育影音","sort":60,"icon_key":"doc","bg_color":"#0ea5e9"},
 {"id":7,"key":"edu_doc","name":"纪录片","sort":70,"icon_key":"doc","bg_color":"#38bdf8"}
]'::jsonb WHERE code='education';

-- game（游戏站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"game_pc","name":"PC游戏","sort":10,"icon_key":"game","bg_color":"#22c55e"},
 {"id":2,"key":"game_console","name":"主机游戏","sort":20,"icon_key":"game","bg_color":"#16a34a"},
 {"id":3,"key":"game_handheld","name":"掌机游戏","sort":30,"icon_key":"game","bg_color":"#4ade80"},
 {"id":4,"key":"game_cn","name":"游戏汉化","sort":40,"icon_key":"game","bg_color":"#86efac"},
 {"id":5,"key":"game_mod","name":"MOD","sort":50,"icon_key":"game","bg_color":"#15803d"},
 {"id":6,"key":"game_ost","name":"游戏音乐","sort":60,"icon_key":"music","bg_color":"#ec4899"},
 {"id":7,"key":"game_guide","name":"攻略","sort":70,"icon_key":"book","bg_color":"#a3a615"}
]'::jsonb WHERE code='game';

-- software（软件站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"soft_windows","name":"Windows","sort":10,"icon_key":"app","bg_color":"#14b8a6"},
 {"id":2,"key":"soft_macos","name":"macOS","sort":20,"icon_key":"app","bg_color":"#0d9488"},
 {"id":3,"key":"soft_linux","name":"Linux","sort":30,"icon_key":"app","bg_color":"#0f766e"},
 {"id":4,"key":"soft_android","name":"Android","sort":40,"icon_key":"app","bg_color":"#2dd4bf"},
 {"id":5,"key":"soft_ios","name":"iOS","sort":50,"icon_key":"app","bg_color":"#5eead4"},
 {"id":6,"key":"soft_tutorial","name":"教程","sort":60,"icon_key":"edu","bg_color":"#16a34a"},
 {"id":7,"key":"soft_asset","name":"素材","sort":70,"icon_key":"app","bg_color":"#99f6e4"}
]'::jsonb WHERE code='software';

-- sports（体育站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"sport_football","name":"足球","sort":10,"icon_key":"sport","bg_color":"#eab308"},
 {"id":2,"key":"sport_basketball","name":"篮球","sort":20,"icon_key":"sport","bg_color":"#d97706"},
 {"id":3,"key":"sport_tennis","name":"网球","sort":30,"icon_key":"sport","bg_color":"#f59e0b"},
 {"id":4,"key":"sport_fight","name":"格斗","sort":40,"icon_key":"sport","bg_color":"#b45309"},
 {"id":5,"key":"sport_racing","name":"赛车","sort":50,"icon_key":"sport","bg_color":"#fbbf24"},
 {"id":6,"key":"sport_mixed","name":"综合赛事","sort":60,"icon_key":"sport","bg_color":"#92400e"},
 {"id":7,"key":"sport_highlights","name":"集锦","sort":70,"icon_key":"tv","bg_color":"#d946ef"}
]'::jsonb WHERE code='sports';

-- documentary（纪录片站）
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"doc_nature","name":"自然","sort":10,"icon_key":"doc","bg_color":"#0ea5e9"},
 {"id":2,"key":"doc_history","name":"历史","sort":20,"icon_key":"doc","bg_color":"#0284c7"},
 {"id":3,"key":"doc_tech","name":"科技","sort":30,"icon_key":"doc","bg_color":"#0369a1"},
 {"id":4,"key":"doc_military","name":"军事","sort":40,"icon_key":"doc","bg_color":"#075985"},
 {"id":5,"key":"doc_humanity","name":"人文","sort":50,"icon_key":"doc","bg_color":"#38bdf8"},
 {"id":6,"key":"doc_adventure","name":"探险","sort":60,"icon_key":"doc","bg_color":"#7dd3fc"},
 {"id":7,"key":"doc_food","name":"美食","sort":70,"icon_key":"doc","bg_color":"#f97316"}
]'::jsonb WHERE code='documentary';

-- 存量物化行回填 key：按「当前包 key ↔ 同名分类」对齐（apply 前的老站，
-- categories 表行名与包声明名一致；对不上的行保持 c<id> 兜底不受伤）
UPDATE categories c SET key = p.cat->>'key', sort = COALESCE((p.cat->>'sort')::int, c.sort),
       icon_key = COALESCE(NULLIF(p.cat->>'icon_key',''), c.icon_key),
       bg_color = COALESCE(p.cat->>'bg_color', c.bg_color)
FROM site_type_packs pk
JOIN site_settings s ON s.name = 'site_type' AND s.value = pk.code,
     jsonb_array_elements(pk.categories) AS p(cat)
WHERE (p.cat->>'id')::int = c.id
  AND c.key LIKE 'c%';

-- ============================================================
-- ③ team 词表补齐（6 个空词表包）+ 标签词表（general/sports/software）
--    口径：追加前剔除同名 ⇒ 幂等；只动包载荷不动 section_dict 表
--    （表在 apply 时按包重建，站点无感的纯数据改动）。
-- ============================================================

-- anime：字幕组（动漫站核心检索实体）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,team}',
  '["Sakura","LoliHouse","北宇治字幕组","幻樱字幕组","DHR動研社","千夏字幕组","ANi"]'::jsonb, true)
WHERE code='anime';

-- general：制作组（综合站泛用口径）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,team}',
  '["OurBits","HDHome","MTeam","HDSky","DBD","CHDBits"]'::jsonb, true)
WHERE code='general';

-- sports：球队/联赛队伍
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,team}',
  '["皇马","巴萨","拜仁","曼联","利物浦","湖人","勇士","国足"]'::jsonb, true)
WHERE code='sports';

-- software：发行组/组织
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,team}',
  '["官方原版","绿色联盟","果核剥壳","PortableAppZ"]'::jsonb, true)
WHERE code='software';

-- documentary：制作方（BBC/NHK 等纪录片厂牌口径）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,team}',
  '["BBC","NHK","Discovery","National Geographic","央视纪录"]'::jsonb, true)
WHERE code='documentary';

-- lossless：发布组（无损音乐站的抓轨/发布组织口径）
UPDATE site_type_packs SET sections = jsonb_set(COALESCE(sections,'{}'::jsonb),'{dict,team}',
  '["EPSILON","HR-Music","iPlus","LoKET"]'::jsonb, true)
WHERE code='lossless';

-- 标签词表（空表三包）
UPDATE site_type_packs SET tags = '[
 {"name":"官方","kind":"official","group":"attribute"},
 {"name":"合集","group":"content"},
 {"name":"完结","group":"attribute"},
 {"name":"独家","group":"attribute"}
]'::jsonb WHERE code='general' AND (tags IS NULL OR tags = '[]'::jsonb);

UPDATE site_type_packs SET tags = '[
 {"name":"官方","kind":"official","group":"attribute"},
 {"name":"全场","group":"content"},
 {"name":"集锦","group":"content"},
 {"name":"录像","group":"content"},
 {"name":"直播源","group":"content"}
]'::jsonb WHERE code='sports' AND (tags IS NULL OR tags = '[]'::jsonb);

UPDATE site_type_packs SET tags = '[
 {"name":"官方","kind":"official","group":"attribute"},
 {"name":"绿色版","group":"attribute"},
 {"name":"开源","group":"attribute"},
 {"name":"汉化","group":"attribute"},
 {"name":"合集","group":"content"}
]'::jsonb WHERE code='software' AND (tags IS NULL OR tags = '[]'::jsonb);

-- ============================================================
-- ④ 元数据源按域重配（H9）：音频站去掉 douban 主源，改 musicbrainz；
--    空源四型 + general 补齐。apply_pack_extras_json 只在包声明 sources
--    数组时写 metadata_sources（幂等），未声明不动站长值。
-- ============================================================
UPDATE site_type_packs SET metadata = '{"sources":["musicbrainz","discogs","mediainfo"]}'::jsonb WHERE code='music';
UPDATE site_type_packs SET metadata = '{"sources":["musicbrainz","discogs","mediainfo"]}'::jsonb WHERE code='lossless';
UPDATE site_type_packs SET metadata = '{"sources":["imdb","douban","bangumi","mediainfo"]}'::jsonb WHERE code='general';
UPDATE site_type_packs SET metadata = '{"sources":["douban","mediainfo"]}'::jsonb WHERE code='ebook';
UPDATE site_type_packs SET metadata = '{"sources":["douban","mediainfo"]}'::jsonb WHERE code='education';
UPDATE site_type_packs SET metadata = '{"sources":["douban","bangumi","mediainfo"]}'::jsonb WHERE code='game';
UPDATE site_type_packs SET metadata = '{"sources":["mediainfo"]}'::jsonb WHERE code='software';
UPDATE site_type_packs SET metadata = '{"sources":["mediainfo"]}'::jsonb WHERE code='sports';

-- ============================================================
-- ⑤ losslessPlayer 死声明清除（H9）：不在 modules 注册表（modules.rs key::ALL
--    / 0107 modules 表 / MODULE_KEYS 三处都没有）、前端零消费者。二选一里
--    先删声明——播放器实装归 music 站型垂直能力批次（总纲阶段 2）。
-- ============================================================
UPDATE site_type_packs SET modules = modules - 'losslessPlayer'
WHERE modules ? 'losslessPlayer';

COMMIT;
