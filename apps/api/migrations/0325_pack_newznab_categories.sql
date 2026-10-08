-- 0325_pack_newznab_categories.sql
-- 站型成熟度 · 批次 3a：分类型 Newznab 分类号进包载荷。
--
-- 现状：categories.newznab_id 只有 general 物化的 10 行配了号；
-- 切到任何专用站型后新增的分类行 newznab_id 全 NULL → Torznab/RSS
-- 出口把这些分类全部回落 Other(8000)——「站长切了音乐站，Prowlarr 里
-- 所有条目都进 Other」。这是对外契约随站型的缺口（对标批次 3）。
--
-- 本迁移：11 包的 categories 载荷补 newznab_id（按 Newznab 标准分类树），
-- apply 时物化进 categories 表（pack_core 的 upsert 已支持写该列——
-- 0317 批 H6 已把 bg_color 接进，newznab_id 同列同路径）。
-- 站长后台改过的号不被覆盖（merge 的 COALESCE 守旧值语义）。
--
-- 幂等：包载荷整段替换（固定值重放等值）。

BEGIN;

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"movie_bd","name":"电影/BluRay","sort":10,"icon_key":"film","bg_color":"#3b82f6","newznab_id":2045},
 {"id":2,"key":"movie_remux","name":"电影/Remux","sort":20,"icon_key":"film","bg_color":"#2563eb","newznab_id":2040},
 {"id":3,"key":"movie_webdl","name":"电影/WEB-DL","sort":30,"icon_key":"film","bg_color":"#1d4ed8","newznab_id":2030},
 {"id":4,"key":"movie_x264","name":"电影/x264","sort":40,"icon_key":"film","bg_color":"#60a5fa","newznab_id":2000},
 {"id":5,"key":"movie_tv","name":"电视剧","sort":50,"icon_key":"tv","bg_color":"#8b5cf6","newznab_id":5040},
 {"id":6,"key":"movie_variety","name":"综艺","sort":60,"icon_key":"tv","bg_color":"#d946ef","newznab_id":5090},
 {"id":7,"key":"movie_doc","name":"纪录片","sort":70,"icon_key":"doc","bg_color":"#0ea5e9","newznab_id":5080}
]'::jsonb WHERE code='movie';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"music_cn","name":"华语音乐","sort":10,"icon_key":"music","bg_color":"#ec4899","newznab_id":3000},
 {"id":2,"key":"music_west","name":"欧美音乐","sort":20,"icon_key":"music","bg_color":"#f472b6","newznab_id":3000},
 {"id":3,"key":"music_jk","name":"日韩音乐","sort":30,"icon_key":"music","bg_color":"#db2777","newznab_id":3000},
 {"id":4,"key":"music_single","name":"单曲","sort":40,"icon_key":"music","bg_color":"#fb7185","newznab_id":3030},
 {"id":5,"key":"music_mv","name":"MV","sort":50,"icon_key":"tv","bg_color":"#d946ef","newznab_id":5030},
 {"id":6,"key":"music_live","name":"演唱会","sort":60,"icon_key":"tv","bg_color":"#c026d3","newznab_id":5090},
 {"id":7,"key":"music_lossless","name":"无损","sort":70,"icon_key":"music","bg_color":"#be185d","newznab_id":3040}
]'::jsonb WHERE code='music';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"lossless_album","name":"无损专辑","sort":10,"icon_key":"music","bg_color":"#be185d","newznab_id":3040},
 {"id":2,"key":"lossless_hires","name":"Hi-Res","sort":20,"icon_key":"music","bg_color":"#a21caf","newznab_id":3040},
 {"id":3,"key":"lossless_vinyl","name":"黑胶转制","sort":30,"icon_key":"music","bg_color":"#86198f","newznab_id":3040},
 {"id":4,"key":"lossless_sacd","name":"SACD","sort":40,"icon_key":"music","bg_color":"#701a75","newznab_id":3040},
 {"id":5,"key":"lossless_sample","name":"采样","sort":50,"icon_key":"music","bg_color":"#9d174d","newznab_id":3000},
 {"id":6,"key":"lossless_single","name":"单曲","sort":60,"icon_key":"music","bg_color":"#fb7185","newznab_id":3030},
 {"id":7,"key":"lossless_live","name":"Live","sort":70,"icon_key":"tv","bg_color":"#c026d3","newznab_id":5090}
]'::jsonb WHERE code='lossless';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"anime_tv","name":"动画","sort":10,"icon_key":"anime","bg_color":"#f97316","newznab_id":5070},
 {"id":2,"key":"anime_season","name":"季度番剧","sort":20,"icon_key":"anime","bg_color":"#ea580c","newznab_id":5070},
 {"id":3,"key":"anime_movie","name":"剧场版","sort":30,"icon_key":"film","bg_color":"#fb923c","newznab_id":5070},
 {"id":4,"key":"anime_manga","name":"漫画","sort":40,"icon_key":"book","bg_color":"#a3a615","newznab_id":7020},
 {"id":5,"key":"anime_doujin","name":"同人","sort":50,"icon_key":"anime","bg_color":"#fdba74","newznab_id":5070},
 {"id":6,"key":"anime_music","name":"音乐","sort":60,"icon_key":"music","bg_color":"#ec4899","newznab_id":3000},
 {"id":7,"key":"anime_goods","name":"周边","sort":70,"icon_key":"app","bg_color":"#f59e0b","newznab_id":8000}
]'::jsonb WHERE code='anime';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"ebook_lit","name":"文学","sort":10,"icon_key":"book","bg_color":"#a3a615","newznab_id":7020},
 {"id":2,"key":"ebook_social","name":"社科","sort":20,"icon_key":"book","bg_color":"#84cc16","newznab_id":7020},
 {"id":3,"key":"ebook_tech","name":"科技","sort":30,"icon_key":"book","bg_color":"#65a30d","newznab_id":7020},
 {"id":4,"key":"ebook_mag","name":"杂志","sort":40,"icon_key":"book","bg_color":"#4d7c0f","newznab_id":7010},
 {"id":5,"key":"ebook_comic","name":"漫画","sort":50,"icon_key":"anime","bg_color":"#f97316","newznab_id":7030},
 {"id":6,"key":"ebook_audio","name":"有声书","sort":60,"icon_key":"music","bg_color":"#ec4899","newznab_id":3030},
 {"id":7,"key":"ebook_guide","name":"教辅","sort":70,"icon_key":"edu","bg_color":"#16a34a","newznab_id":7020}
]'::jsonb WHERE code='ebook';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"edu_preschool","name":"学前教育","sort":10,"icon_key":"edu","bg_color":"#16a34a","newznab_id":7020},
 {"id":2,"key":"edu_primary","name":"小学","sort":20,"icon_key":"edu","bg_color":"#22c55e","newznab_id":7020},
 {"id":3,"key":"edu_junior","name":"初中","sort":30,"icon_key":"edu","bg_color":"#15803d","newznab_id":7020},
 {"id":4,"key":"edu_vocational","name":"职高","sort":40,"icon_key":"edu","bg_color":"#4ade80","newznab_id":7020},
 {"id":5,"key":"edu_senior","name":"高中","sort":50,"icon_key":"edu","bg_color":"#166534","newznab_id":7020},
 {"id":6,"key":"edu_media","name":"教育影音","sort":60,"icon_key":"doc","bg_color":"#0ea5e9","newznab_id":5080},
 {"id":7,"key":"edu_doc","name":"纪录片","sort":70,"icon_key":"doc","bg_color":"#38bdf8","newznab_id":5080}
]'::jsonb WHERE code='education';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"game_pc","name":"PC游戏","sort":10,"icon_key":"game","bg_color":"#22c55e","newznab_id":4050},
 {"id":2,"key":"game_console","name":"主机游戏","sort":20,"icon_key":"game","bg_color":"#16a34a","newznab_id":100010},
 {"id":3,"key":"game_handheld","name":"掌机游戏","sort":30,"icon_key":"game","bg_color":"#4ade80","newznab_id":100020},
 {"id":4,"key":"game_cn","name":"游戏汉化","sort":40,"icon_key":"game","bg_color":"#86efac","newznab_id":4050},
 {"id":5,"key":"game_mod","name":"MOD","sort":50,"icon_key":"game","bg_color":"#15803d","newznab_id":4050},
 {"id":6,"key":"game_ost","name":"游戏音乐","sort":60,"icon_key":"music","bg_color":"#ec4899","newznab_id":3000},
 {"id":7,"key":"game_guide","name":"攻略","sort":70,"icon_key":"book","bg_color":"#a3a615","newznab_id":7020}
]'::jsonb WHERE code='game';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"soft_windows","name":"Windows","sort":10,"icon_key":"app","bg_color":"#14b8a6","newznab_id":4000},
 {"id":2,"key":"soft_macos","name":"macOS","sort":20,"icon_key":"app","bg_color":"#0d9488","newznab_id":4010},
 {"id":3,"key":"soft_linux","name":"Linux","sort":30,"icon_key":"app","bg_color":"#0f766e","newznab_id":4020},
 {"id":4,"key":"soft_android","name":"Android","sort":40,"icon_key":"app","bg_color":"#2dd4bf","newznab_id":4030},
 {"id":5,"key":"soft_ios","name":"iOS","sort":50,"icon_key":"app","bg_color":"#5eead4","newznab_id":4040},
 {"id":6,"key":"soft_tutorial","name":"教程","sort":60,"icon_key":"edu","bg_color":"#16a34a","newznab_id":8000},
 {"id":7,"key":"soft_asset","name":"素材","sort":70,"icon_key":"app","bg_color":"#99f6e4","newznab_id":8000}
]'::jsonb WHERE code='software';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"sport_football","name":"足球","sort":10,"icon_key":"sport","bg_color":"#eab308","newznab_id":5060},
 {"id":2,"key":"sport_basketball","name":"篮球","sort":20,"icon_key":"sport","bg_color":"#d97706","newznab_id":5060},
 {"id":3,"key":"sport_tennis","name":"网球","sort":30,"icon_key":"sport","bg_color":"#f59e0b","newznab_id":5060},
 {"id":4,"key":"sport_fight","name":"格斗","sort":40,"icon_key":"sport","bg_color":"#b45309","newznab_id":5060},
 {"id":5,"key":"sport_racing","name":"赛车","sort":50,"icon_key":"sport","bg_color":"#fbbf24","newznab_id":5060},
 {"id":6,"key":"sport_mixed","name":"综合赛事","sort":60,"icon_key":"sport","bg_color":"#92400e","newznab_id":5060},
 {"id":7,"key":"sport_highlights","name":"集锦","sort":70,"icon_key":"tv","bg_color":"#d946ef","newznab_id":5090}
]'::jsonb WHERE code='sports';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"doc_nature","name":"自然","sort":10,"icon_key":"doc","bg_color":"#0ea5e9","newznab_id":5080},
 {"id":2,"key":"doc_history","name":"历史","sort":20,"icon_key":"doc","bg_color":"#0284c7","newznab_id":5080},
 {"id":3,"key":"doc_tech","name":"科技","sort":30,"icon_key":"doc","bg_color":"#0369a1","newznab_id":5080},
 {"id":4,"key":"doc_military","name":"军事","sort":40,"icon_key":"doc","bg_color":"#075985","newznab_id":5080},
 {"id":5,"key":"doc_humanity","name":"人文","sort":50,"icon_key":"doc","bg_color":"#38bdf8","newznab_id":5080},
 {"id":6,"key":"doc_adventure","name":"探险","sort":60,"icon_key":"doc","bg_color":"#7dd3fc","newznab_id":5080},
 {"id":7,"key":"doc_food","name":"美食","sort":70,"icon_key":"doc","bg_color":"#f97316","newznab_id":5080}
]'::jsonb WHERE code='documentary';

UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"general_movie","name":"电影","sort":10,"icon_key":"film","bg_color":"#3b82f6","newznab_id":2000},
 {"id":2,"key":"general_tv","name":"电视剧","sort":20,"icon_key":"tv","bg_color":"#8b5cf6","newznab_id":5000},
 {"id":3,"key":"general_music","name":"音乐","sort":30,"icon_key":"music","bg_color":"#ec4899","newznab_id":3000},
 {"id":4,"key":"general_game","name":"游戏","sort":40,"icon_key":"game","bg_color":"#22c55e","newznab_id":4050},
 {"id":5,"key":"general_software","name":"软件","sort":50,"icon_key":"app","bg_color":"#14b8a6","newznab_id":4000},
 {"id":6,"key":"general_anime","name":"动漫","sort":60,"icon_key":"anime","bg_color":"#f97316","newznab_id":5070},
 {"id":7,"key":"general_doc","name":"纪录片","sort":70,"icon_key":"doc","bg_color":"#0ea5e9","newznab_id":5080},
 {"id":8,"key":"general_ebook","name":"电子书","sort":80,"icon_key":"book","bg_color":"#a3a615","newznab_id":7020},
 {"id":9,"key":"general_sports","name":"体育","sort":90,"icon_key":"sport","bg_color":"#eab308","newznab_id":5060},
 {"id":10,"key":"general_variety","name":"综艺","sort":100,"icon_key":"tv","bg_color":"#d946ef","newznab_id":5090}
]'::jsonb WHERE code='general';

-- 存量物化行回填（只补 NULL，不动站长改过的号）
UPDATE categories c SET newznab_id = (p.cat->>'newznab_id')::int
FROM site_type_packs pk
JOIN site_settings s ON s.name = 'site_type' AND s.value = pk.code,
     jsonb_array_elements(pk.categories) AS p(cat)
WHERE (p.cat->>'key') IS NOT NULL AND p.cat->>'key' = c.key
  AND c.newznab_id IS NULL
  AND p.cat ? 'newznab_id';

COMMIT;
