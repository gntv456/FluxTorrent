-- 0443: 制作组（team）词表重写——收敛为 PT 官组后缀口径。
--
-- 背景（2026-10-31 站长定调）：0441 补的词表把论坛站名（人人影视/伊甸园/
-- 电影天堂…）、SCENE/流媒体标签（AMZN/NF/RARBG…）、游戏破解组（CODEX/
-- FitGirl…）、软件版本词（绿色版/便携版…）、电视台（BBC/CCTV-9…）、体育
-- （NBA/FIFA…）、抓轨动词（自抓/自购/自压）全部混进「制作组」。制作组的
-- 正确定义 = **各 PT 站官组后缀**（如 HX/HXWEB/HXPT、CSPT/CSWEB 这类
-- 站名缩写系），外加通用语义位（官方/自制/转载/其他）。
--
-- 引用体检（2026-10-31 服务器实测）：team 引用仅 2 条（MTeam/CMCT，均在
-- 保留名单）；无引用的行直接删。
--
-- 修法（幂等三步）：
--   ① 删非官组行：论坛站名/SCENE 流媒体/游戏/软件/电视台/体育/音乐块
--      （193 行）+ 动漫块中英双名合一（保留英文名——PT 命名后缀惯例，
--      删中文名变体：喵萌奶茶屋=Nekomoe kissaten 等）；
--   ② 补本站系官组（HX/HXWEB/HXPT/CSPT/CSWEB 口径的自家缩写，站长可
--      在后台继续增补其他站的官组）；
--   ③ 站型包 sections.dict.team 同步为新名单（防切站型还原旧表）。

-- ===== ① 删非官组行（全部无引用；删除前后引用数守恒由 ② 前的体检保证） =====
DELETE FROM torrent_sections
 WHERE kind = 'team' AND dict_id IN (SELECT id FROM section_dict
   WHERE kind = 'team' AND name IN (
  -- 论坛站名（不是官组）
  '人人影视','伊甸园','悠悠鸟','圣城家园','电影天堂','BT天堂','飘花电影',
  '6v电影','高清范','4K家园','好4K','蓝光原盘',
  -- SCENE / 流媒体标签（是来源标签不是制作组；来源维度另有 source kind）
  'SPARKS','AMZN','NF','iTunes','HMAX','HULU','DSNP','MA','EVO','RARBG',
  'YIFY','YTS','FGT','GECKOS','CMRG','DEFLATE','NTb','TEPES','HONE','SA89',
  'PLAYREADY','KOGi','XLR','HYPE','HDO','NTG','EPSiLON','ESiR','HiFi','iFT',
  'KRaLiMaRKo','playBD','SumVision',
  -- 游戏破解组（游戏区自有 release 生态，不混影视官组）
  'CODEX','PLAZA','SKIDROW','RELOADED','CPY','EMPRESS','GOG','FLT','TiNYiSO',
  'DARKSiDERS','Razor1911','Hoodlum','PROPHET','RUNE','TENOKE','ElAmigos',
  'FitGirl','DODI','Goldberg','3DM','游侠','轩辕','汉化组','破解组','整合组',
  -- 软件版本词（是属性不是组）
  '绿色版','便携版','整合版','破解版','官方原版',
  -- 电视台/纪录片出品方（不是压制组）
  'BBC','NHK','PBS','National Geographic','Discovery','History Channel',
  'ZDF','Arte','Netflix','Apple TV+','央视纪录','CCTV-9','B站出品','腾讯视频',
  -- 体育
  '电视台','流媒体','全场录像','CCTV-5','咪咕视频','腾讯体育','爱奇艺体育',
  'NBA','FIFA','UEFA',
  -- 音乐抓轨动词
  '自抓','自购','自压',
  -- 动漫块中文名变体（英文名保留，PT 后缀惯例）
  '喵萌奶茶屋','桜都字幕组','澄空学园','华盟字幕社','动漫国字幕组',
  '极影字幕社','轻之国度','天使动漫','千夏字幕组','幻樱字幕组',
  '银铃字幕组','星空字幕组','拨雪寻春','北宇治字幕组','豌豆字幕组',
  '悠哈璃羽字幕社','白恋字幕组','三明治','森野','御所','夜桜字幕组',
  '风之圣殿','雪飘工作室','诸神字幕组','动音漫影','异域字幕组',
  '红旅动漫','漫游字幕组','爱恋字幕社','动漫花园'));

DELETE FROM section_dict
 WHERE kind = 'team' AND name IN (
  '人人影视','伊甸园','悠悠鸟','圣城家园','电影天堂','BT天堂','飘花电影',
  '6v电影','高清范','4K家园','好4K','蓝光原盘',
  'SPARKS','AMZN','NF','iTunes','HMAX','HULU','DSNP','MA','EVO','RARBG',
  'YIFY','YTS','FGT','GECKOS','CMRG','DEFLATE','NTb','TEPES','HONE','SA89',
  'PLAYREADY','KOGi','XLR','HYPE','HDO','NTG','EPSiLON','ESiR','HiFi','iFT',
  'KRaLiMaRKo','playBD','SumVision',
  'CODEX','PLAZA','SKIDROW','RELOADED','CPY','EMPRESS','GOG','FLT','TiNYiSO',
  'DARKSiDERS','Razor1911','Hoodlum','PROPHET','RUNE','TENOKE','ElAmigos',
  'FitGirl','DODI','Goldberg','3DM','游侠','轩辕','汉化组','破解组','整合组',
  '绿色版','便携版','整合版','破解版','官方原版',
  'BBC','NHK','PBS','National Geographic','Discovery','History Channel',
  'ZDF','Arte','Netflix','Apple TV+','央视纪录','CCTV-9','B站出品','腾讯视频',
  '电视台','流媒体','全场录像','CCTV-5','咪咕视频','腾讯体育','爱奇艺体育',
  'NBA','FIFA','UEFA',
  '自抓','自购','自压',
  '喵萌奶茶屋','桜都字幕组','澄空学园','华盟字幕社','动漫国字幕组',
  '极影字幕社','轻之国度','天使动漫','千夏字幕组','幻樱字幕组',
  '银铃字幕组','星空字幕组','拨雪寻春','北宇治字幕组','豌豆字幕组',
  '悠哈璃羽字幕社','白恋字幕组','三明治','森野','御所','夜桜字幕组',
  '风之圣殿','雪飘工作室','诸神字幕组','动音漫影','异域字幕组',
  '红旅动漫','漫游字幕组','爱恋字幕社','动漫花园');

-- ===== ② 补本站系官组缩写（HX/CS 口径；站长可在后台继续加别站官组） =====
INSERT INTO section_dict (kind, name, sort)
SELECT 'team', v.n, v.s FROM (VALUES
  ('HX',     3), ('HXWEB',  4), ('HXPT',  5),
  ('CSPT',   6), ('CSWEB',  7)
) AS v(n, s)
WHERE NOT EXISTS (SELECT 1 FROM section_dict d
                   WHERE d.kind = 'team' AND d.name = v.n);

-- ===== ③ 站型包同步（重放/切站型不得回灌旧词表） =====
UPDATE site_type_packs p
   SET sections = jsonb_set(
         p.sections, '{dict,team}',
         (SELECT jsonb_agg(d.name ORDER BY d.sort)
            FROM section_dict d WHERE d.kind = 'team'),
         TRUE)
 WHERE p.sections ? 'dict';
