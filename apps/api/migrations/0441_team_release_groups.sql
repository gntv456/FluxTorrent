-- 0441: 制作组（team）词表补齐
--
-- 现状：section_kinds.team 是自由文本，section_dict(kind='team') 0 行；
-- 转种/发布时只能手填，自动转种脚本按字典匹配时拿不到选项，制作组常空着。
-- 本迁移：
--   1) 补一份跨影视/动画/游戏/软件/纪录片/体育的常用制作组/压制组词表；
--   2) 把 team 收敛为单选下拉（有存量自由值时先转成字典行，不丢数据）；
--   3) 同步站型包 sections.dict.team，避免切/重放站型时把词表清空。
-- 只追加字典行，不覆盖站长以后自行添加的选项。

DROP TABLE IF EXISTS _team_seed;
CREATE TEMP TABLE _team_seed (name TEXT PRIMARY KEY, sort INT NOT NULL);

INSERT INTO _team_seed (name, sort) VALUES
  -- 通用
  ('官方', 1), ('自制', 2), ('转载', 3), ('其他', 9999),
  -- 影视/PT 压制与发布组
  ('WiKi', 10), ('CMCT', 11), ('FRDS', 12), ('HDSky', 13),
  ('HDChina', 14), ('CHDBits', 15), ('TTG', 16), ('OurBits', 17),
  ('HDHome', 18), ('HDArea', 19), ('HDRoute', 20), ('HDSpace', 21),
  ('HDTime', 22), ('HDZone', 23), ('HDCity', 24), ('HDStar', 25),
  ('HDRoad', 26), ('HD001', 27), ('HDSource', 28), ('HDCTV', 29),
  ('HDFans', 30), ('HDU', 31), ('HDKing', 32), ('HDB', 33),
  ('HDW', 34), ('HDC', 35), ('CHD', 36), ('MySiLU', 37),
  ('HDS', 38), ('HDA', 39), ('HDT', 40), ('TLF', 41),
  ('NGB', 42), ('QHstudIo', 43), ('SF', 44), ('HDV', 45),
  ('HDM', 46), ('3E', 47), ('DiY', 48), ('SubHD', 49),
  ('人人影视', 50), ('伊甸园', 51), ('悠悠鸟', 52), ('圣城家园', 53),
  ('电影天堂', 54), ('BT天堂', 55), ('飘花电影', 56), ('6v电影', 57),
  ('高清范', 58), ('4K家园', 59), ('好4K', 60), ('蓝光原盘', 61),
  ('FLUX', 62), ('PandaMoon', 63), ('HDClub', 64), ('LCHD', 65),
  ('BMDru', 66), ('MPAD', 67), ('TNP', 68), ('MWEB', 69),
  -- 常见 SCENE / 流媒体组
  ('SPARKS', 100), ('AMZN', 101), ('NF', 102), ('iTunes', 103),
  ('HMAX', 104), ('HULU', 105), ('DSNP', 106), ('MA', 107),
  ('EVO', 108), ('RARBG', 109), ('YIFY', 110), ('YTS', 111),
  ('FGT', 112), ('GECKOS', 113), ('CMRG', 114), ('DEFLATE', 115),
  ('NTb', 116), ('TEPES', 117), ('HONE', 118), ('SA89', 119),
  ('PLAYREADY', 120), ('KOGi', 121), ('XLR', 122), ('HYPE', 123),
  ('HDO', 124), ('NTG', 125), ('EPSiLON', 126), ('ESiR', 127),
  ('HiFi', 128), ('iFT', 129), ('KRaLiMaRKo', 130), ('playBD', 131),
  ('SumVision', 132),
  -- 动画/字幕/压制组
  ('LoliHouse', 200), ('SweetSub', 201), ('喵萌奶茶屋', 202),
  ('桜都字幕组', 203), ('澄空学园', 204), ('华盟字幕社', 205),
  ('动漫国字幕组', 206), ('极影字幕社', 207), ('轻之国度', 208),
  ('天使动漫', 209), ('千夏字幕组', 210), ('幻樱字幕组', 211),
  ('银铃字幕组', 212), ('星空字幕组', 213), ('拨雪寻春', 214),
  ('北宇治字幕组', 215), ('豌豆字幕组', 216), ('悠哈璃羽字幕社', 217),
  ('白恋字幕组', 218), ('三明治', 219), ('森野', 220),
  ('御所', 221), ('夜桜字幕组', 222), ('风之圣殿', 223),
  ('雪飘工作室', 224), ('诸神字幕组', 225), ('动音漫影', 226),
  ('异域字幕组', 227), ('红旅动漫', 228), ('漫游字幕组', 229),
  ('爱恋字幕社', 230), ('动漫花园', 231), ('VCB-Studio', 232),
  ('Moozzi2', 233), ('ANi', 234), ('Airota', 235),
  ('Ohys-Raws', 236), ('Beatrice-Raws', 237), ('ReinForce', 238),
  ('Kagura', 239), ('ANK-Raws', 240), ('AI-RAWS', 241),
  ('Nekomoe kissaten', 242), ('Sakurato', 243),
  -- 游戏
  ('CODEX', 300), ('PLAZA', 301), ('SKIDROW', 302),
  ('RELOADED', 303), ('CPY', 304), ('EMPRESS', 305),
  ('GOG', 306), ('FLT', 307), ('TiNYiSO', 308),
  ('DARKSiDERS', 309), ('Razor1911', 310), ('Hoodlum', 311),
  ('PROPHET', 312), ('RUNE', 313), ('TENOKE', 314),
  ('ElAmigos', 315), ('FitGirl', 316), ('DODI', 317),
  ('Goldberg', 318), ('3DM', 319), ('游侠', 320),
  ('轩辕', 321), ('汉化组', 322), ('破解组', 323),
  ('整合组', 324),
  -- 软件
  ('绿色版', 400), ('便携版', 401), ('整合版', 402),
  ('破解版', 403), ('官方原版', 404),
  -- 纪录片/电视台
  ('BBC', 500), ('NHK', 501), ('PBS', 502),
  ('National Geographic', 503), ('Discovery', 504),
  ('History Channel', 505), ('ZDF', 506), ('Arte', 507),
  ('Netflix', 508), ('Apple TV+', 509), ('央视纪录', 510),
  ('CCTV-9', 511), ('B站出品', 512), ('腾讯视频', 513),
  -- 体育
  ('电视台', 600), ('流媒体', 601), ('全场录像', 602),
  ('CCTV-5', 603), ('咪咕视频', 604), ('腾讯体育', 605),
  ('爱奇艺体育', 606), ('NBA', 607), ('FIFA', 608), ('UEFA', 609),
  -- 音乐抓轨/发行
  ('自抓', 700), ('自购', 701), ('自压', 702);

-- 1) 追加缺失选项（保留站长已自建的 team 行）
INSERT INTO section_dict (kind, name, sort)
SELECT 'team', s.name, s.sort
  FROM _team_seed s
 WHERE NOT EXISTS (
   SELECT 1 FROM section_dict d
    WHERE d.kind = 'team' AND d.name = s.name
 );

-- 2) 有存量自由值时先落字典，再切下拉；空/非字符串脏值保留原类型
INSERT INTO section_dict (kind, name, sort)
SELECT DISTINCT 'team', BTRIM(ts.value #>> '{}'), 900
  FROM torrent_sections ts
 WHERE ts.kind = 'team'
   AND ts.value IS NOT NULL
   AND jsonb_typeof(ts.value) = 'string'
   AND BTRIM(ts.value #>> '{}') <> ''
   AND NOT EXISTS (
     SELECT 1 FROM section_dict d
      WHERE d.kind = 'team' AND d.name = BTRIM(ts.value #>> '{}')
   );

UPDATE torrent_sections ts
   SET dict_id = d.id, value = NULL
  FROM section_dict d
 WHERE ts.kind = 'team'
   AND ts.value IS NOT NULL
   AND jsonb_typeof(ts.value) = 'string'
   AND d.kind = 'team'
   AND d.name = BTRIM(ts.value #>> '{}');

UPDATE section_kinds
   SET field_type = 'select', multiple = FALSE
 WHERE kind = 'team'
   AND NOT EXISTS (
     SELECT 1 FROM torrent_sections ts
      WHERE ts.kind = 'team' AND ts.value IS NOT NULL
   );

-- 3) 站型包同步：重放/切换站型时不再把 team 词表清空
UPDATE site_type_packs p
   SET sections = jsonb_set(
         p.sections, '{dict,team}',
         (SELECT jsonb_agg(s.name ORDER BY s.sort) FROM _team_seed s),
         TRUE)
 WHERE p.sections ? 'dict';

UPDATE site_type_packs p
   SET sections = jsonb_set(
         p.sections, '{kinds}',
         (SELECT jsonb_agg(
                   CASE WHEN e->>'kind' = 'team'
                        THEN e || '{"field_type":"select"}'::jsonb
                        ELSE e END
                   ORDER BY ord)
            FROM jsonb_array_elements(p.sections->'kinds')
                 WITH ORDINALITY AS x(e, ord)),
         TRUE)
 WHERE p.sections ? 'kinds'
   AND EXISTS (
     SELECT 1 FROM jsonb_array_elements(p.sections->'kinds') e
      WHERE e->>'kind' = 'team'
   );

DROP TABLE IF EXISTS _team_seed;
