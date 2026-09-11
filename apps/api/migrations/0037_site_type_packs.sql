-- 0037 通用 PT 站点类型系统（site-type packs）
-- 目标：把「教育站」假设解耦成可切换的类型包（教育/影视/音乐/动漫/电子书/综合/体育/游戏/软件/纪录片/成人/无损音乐），
-- 对标 NexusPHP/Gazelle/meanTorrent 的可配置站点属性。

-- 类型包注册表（categories 快照 + 模块开关 + 品牌默认值）
CREATE TABLE IF NOT EXISTS site_type_packs (
    code        TEXT PRIMARY KEY,           -- education / movie / music / anime / ebook / general / sports / game / software / documentary / lossless
    name        TEXT NOT NULL,
    description TEXT,
    brand       TEXT NOT NULL DEFAULT '',   -- 默认站名
    categories  JSONB NOT NULL,             -- [{id,name}]
    modules     JSONB NOT NULL,             -- {textbooks:bool, losslessPlayer:bool, showcase:bool, ...}
    sort        INT NOT NULL DEFAULT 0
);

INSERT INTO site_type_packs (code, name, description, brand, categories, modules, sort)
SELECT * FROM (VALUES
  ('education', '教育站', 'K12/高教资源', '包子PT',
   '[{"id":1,"name":"学前教育"},{"id":2,"name":"小学"},{"id":3,"name":"初中"},{"id":4,"name":"职高"},{"id":5,"name":"高中"},{"id":6,"name":"教育影音"},{"id":7,"name":"纪录片"}]'::jsonb,
   '{"textbooks":true}'::jsonb, 1),
  ('movie', '影视站', '电影/剧集/综艺（馒头口径）', '影站',
   '[{"id":1,"name":"电影/BluRay"},{"id":2,"name":"电影/Remux"},{"id":3,"name":"电影/WEB-DL"},{"id":4,"name":"电影/x264"},{"id":5,"name":"电视剧"},{"id":6,"name":"综艺"},{"id":7,"name":"纪录片"}]'::jsonb,
   '{"textbooks":false,"showcase":true}'::jsonb, 2),
  ('music', '音乐站', '流行/单曲/MV', '乐站',
   '[{"id":1,"name":"华语音乐"},{"id":2,"name":"欧美音乐"},{"id":3,"name":"日韩音乐"},{"id":4,"name":"单曲"},{"id":5,"name":"MV"},{"id":6,"name":"演唱会"},{"id":7,"name":"无损"}]'::jsonb,
   '{"textbooks":false,"losslessPlayer":true}'::jsonb, 3),
  ('anime', '动漫站', '动画/漫画/生肉/熟肉', '漫站',
   '[{"id":1,"name":"动画"},{"id":2,"name":"季度番剧"},{"id":3,"name":"剧场版"},{"id":4,"name":"漫画"},{"id":5,"name":"同人"},{"id":6,"name":"音乐"},{"id":7,"name":"周边"}]'::jsonb,
   '{"textbooks":false}'::jsonb, 4),
  ('ebook', '电子书站', '图书/杂志/漫画/有声书', '书站',
   '[{"id":1,"name":"文学"},{"id":2,"name":"社科"},{"id":3,"name":"科技"},{"id":4,"name":"杂志"},{"id":5,"name":"漫画"},{"id":6,"name":"有声书"},{"id":7,"name":"教辅"}]'::jsonb,
   '{"textbooks":false}'::jsonb, 5),
  ('general', '综合站', '全品类（NexusPHP 默认）', '综合站',
   '[{"id":1,"name":"电影"},{"id":2,"name":"电视剧"},{"id":3,"name":"音乐"},{"id":4,"name":"游戏"},{"id":5,"name":"软件"},{"id":6,"name":"动漫"},{"id":7,"name":"纪录片"},{"id":8,"name":"电子书"},{"id":9,"name":"体育"},{"id":10,"name":"综艺"}]'::jsonb,
   '{}'::jsonb, 6),
  ('sports', '体育站', '足球/篮球/赛事', '体站',
   '[{"id":1,"name":"足球"},{"id":2,"name":"篮球"},{"id":3,"name":"网球"},{"id":4,"name":"格斗"},{"id":5,"name":"赛车"},{"id":6,"name":"综合赛事"},{"id":7,"name":"集锦"}]'::jsonb,
   '{"textbooks":false}'::jsonb, 7),
  ('game', '游戏站', 'PC/主机/掌机资源', '游站',
   '[{"id":1,"name":"PC游戏"},{"id":2,"name":"主机游戏"},{"id":3,"name":"掌机游戏"},{"id":4,"name":"游戏汉化"},{"id":5,"name":"MOD"},{"id":6,"name":"游戏音乐"},{"id":7,"name":"攻略"}]'::jsonb,
   '{"textbooks":false}'::jsonb, 8),
  ('software', '软件站', '软件/系统/教程', '软站',
   '[{"id":1,"name":"Windows"},{"id":2,"name":"macOS"},{"id":3,"name":"Linux"},{"id":4,"name":"Android"},{"id":5,"name":"iOS"},{"id":6,"name":"教程"},{"id":7,"name":"素材"}]'::jsonb,
   '{"textbooks":false}'::jsonb, 9),
  ('documentary', '纪录片站', '纪录片/自然/历史/科技', '纪录站',
   '[{"id":1,"name":"自然"},{"id":2,"name":"历史"},{"id":3,"name":"科技"},{"id":4,"name":"军事"},{"id":5,"name":"人文"},{"id":6,"name":"探险"},{"id":7,"name":"美食"}]'::jsonb,
   '{"textbooks":false}'::jsonb, 10),
  ('lossless', '无损音乐站', '无损/Hi-Res/采样（Gazelle 口径）', '无损站',
   '[{"id":1,"name":"无损专辑"},{"id":2,"name":"Hi-Res"},{"id":3,"name":"黑胶转制"},{"id":4,"name":"SACD"},{"id":5,"name":"采样"},{"id":6,"name":"单曲"},{"id":7,"name":"Live"}]'::jsonb,
   '{"textbooks":false,"losslessPlayer":true}'::jsonb, 11)
) AS seed(code, name, description, brand, categories, modules, sort)
WHERE NOT EXISTS (SELECT 1 FROM site_type_packs);

-- 当前站点类型（默认 education，兼容现状）
INSERT INTO site_settings (name, value, descr)
VALUES ('site_type', 'education', '站点类型包（education/movie/music/.../lossless）')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;
