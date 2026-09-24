-- 0178：通用建站定位收口（一审 R1/R9 + 二审 R6/R7 配套迁移）。
--
-- 1) 模块缺省翻转：注册表缺省从「教育站形态全开」改为「中立最小集」——
--    核心层（种子/认证/计费）本就不在注册表内；注册表 29 键的缺省一律由
--    站型包矩阵表达（setup/apply 时落 module_*），裸库不再默认长出教育/娱乐形态。
--    11 个预置包补齐**完整 29 键** modules 快照（原 0107 只有部分覆盖）。
-- 2) 运营内容中性化：初装论坛版块/勋章/任务/成就/趣味投票里的教育口吻
--    一律改为站型中立文案（守卫式：仅当仍为默认值才改，站长改过的不动）。
-- 3) 自定义站型包补 subtitle_kind 列（二审 G7d：music/lossless 另存 custom 包
--    再 apply 不再把歌词口径重置回影视字幕）。
-- 4) apply_subtitle_kind_explicit(is_lyric)：按包快照显式应用字幕口径，
--    custom_* 前缀不再靠 code 猜。

-- ============ 1) 模块矩阵：11 包全量 29 键 ============
-- general（综合站基准）：社区/经济/基础运营全开，教育考核与重度娱乐关
UPDATE site_type_packs SET modules = '{
  "forums":true,"messages":true,"friends":true,"offers":true,"requests":true,
  "subtitles":true,"preserve":true,"shoutbox":true,
  "promo_buy":true,"bank":true,"shop":true,"magic_pool":true,"vouchers":true,
  "resurrections":true,"wishlist":true,"games":true,
  "attendance":true,"medals":true,"dressup":true,"tasks":true,"push":true,
  "textbooks":false,"showcase":false,"social":false,"farm":false,
  "gomoku":false,"contests":false,"jixiao":false,"exams":false
}'::jsonb WHERE code = 'general';

UPDATE site_type_packs SET modules = '{
  "forums":true,"messages":true,"friends":true,"offers":true,"requests":true,
  "subtitles":true,"preserve":true,"shoutbox":true,
  "promo_buy":true,"bank":true,"shop":true,"magic_pool":true,"vouchers":true,
  "resurrections":true,"wishlist":true,"games":true,
  "attendance":true,"medals":true,"dressup":true,"tasks":true,"push":true,
  "textbooks":true,"showcase":false,"social":false,"farm":true,
  "gomoku":true,"contests":false,"jixiao":true,"exams":true
}'::jsonb WHERE code = 'education';

UPDATE site_type_packs SET modules = '{
  "forums":true,"messages":true,"friends":true,"offers":true,"requests":true,
  "subtitles":true,"preserve":true,"shoutbox":true,
  "promo_buy":true,"bank":true,"shop":true,"magic_pool":true,"vouchers":true,
  "resurrections":true,"wishlist":true,"games":true,
  "attendance":true,"medals":true,"dressup":true,"tasks":true,"push":true,
  "textbooks":false,"showcase":true,"social":false,"farm":false,
  "gomoku":false,"contests":false,"jixiao":false,"exams":false
}'::jsonb WHERE code IN ('movie', 'documentary');

UPDATE site_type_packs SET modules = '{
  "forums":true,"messages":true,"friends":true,"offers":true,"requests":true,
  "subtitles":true,"preserve":true,"shoutbox":true,
  "promo_buy":true,"bank":true,"shop":true,"magic_pool":true,"vouchers":true,
  "resurrections":true,"wishlist":true,"games":true,
  "attendance":true,"medals":true,"dressup":true,"tasks":true,"push":true,
  "textbooks":false,"showcase":false,"social":false,"farm":false,
  "gomoku":false,"contests":false,"jixiao":false,"exams":false
}'::jsonb WHERE code IN ('music', 'lossless', 'anime', 'software');

UPDATE site_type_packs SET modules = '{
  "forums":true,"messages":true,"friends":true,"offers":true,"requests":true,
  "subtitles":true,"preserve":true,"shoutbox":true,
  "promo_buy":true,"bank":true,"shop":true,"magic_pool":true,"vouchers":true,
  "resurrections":true,"wishlist":true,"games":true,
  "attendance":true,"medals":true,"dressup":true,"tasks":true,"push":true,
  "textbooks":true,"showcase":false,"social":false,"farm":false,
  "gomoku":false,"contests":false,"jixiao":false,"exams":false
}'::jsonb WHERE code = 'ebook';

UPDATE site_type_packs SET modules = '{
  "forums":true,"messages":true,"friends":true,"offers":true,"requests":true,
  "subtitles":true,"preserve":true,"shoutbox":true,
  "promo_buy":true,"bank":true,"shop":true,"magic_pool":true,"vouchers":true,
  "resurrections":true,"wishlist":true,"games":true,
  "attendance":true,"medals":true,"dressup":true,"tasks":true,"push":true,
  "textbooks":false,"showcase":false,"social":false,"farm":false,
  "gomoku":false,"contests":true,"jixiao":false,"exams":false
}'::jsonb WHERE code IN ('sports', 'game');

-- ============ 2) 注册表缺省与站点键对齐 general 基准 ============
-- modules.is_on（缺省语义权威）与 site_settings.module_*（运行时权威）
-- 一并对齐 general 矩阵：裸库/漏键实例不再呈现教育站形态。
UPDATE modules SET is_on = TRUE WHERE key IN (
  'forums','messages','friends','offers','requests','subtitles','preserve',
  'shoutbox','promo_buy','bank','shop','magic_pool','vouchers','resurrections',
  'wishlist','games','attendance','medals','dressup','tasks','push');
UPDATE modules SET is_on = FALSE WHERE key IN (
  'textbooks','showcase','social','farm','gomoku','contests','jixiao','exams');

INSERT INTO site_settings (name, value, descr, grp)
SELECT 'module_' || key, CASE WHEN is_on THEN 'yes' ELSE 'no' END,
       (SELECT name_zh FROM modules m2 WHERE m2.key = modules.key) || '（模块开关）',
       'module'
FROM modules
ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();

-- ============ 3) 运营内容中性化（守卫式，站长改过的不动） ============
UPDATE forums SET name = '资源讨论', descr = '资源与学习方法交流'
WHERE name = '课本讨论';
UPDATE medals SET name = '活跃先锋' WHERE name = '开学先锋';
UPDATE medals SET name = '夏日清凉' WHERE name = '放暑假啦';
UPDATE medals SET name = '互动之星' WHERE name = '学习之星';
UPDATE tasks SET name = replace(name, '精进研习社', '做种研习社')
WHERE name LIKE '精进研习社%';
UPDATE achievement_defs SET name = '首度发布',
  descr = '过审发布 ≥ 1 个' WHERE code = 'up_1' AND name = '初为人师';
UPDATE achievement_defs SET name = '发布达人',
  descr = '过审发布 ≥ 50 个' WHERE code = 'up_50' AND name = '桃李天下';
UPDATE fun_polls SET options = replace(options::text, '正版教科书', '正版软件')::jsonb
WHERE options::text LIKE '%正版教科书%';

-- education 包 brand 仍是「包子PT」（0118 只清空了非 education 包）：
-- 预置包品牌一律空串回落 FluxTorrent，站名由站长/向导决定
UPDATE site_type_packs SET brand = '' WHERE code = 'education' AND brand = '包子PT';

-- ============ 4) 自定义站型包：subtitle_kind 快照列 ============
ALTER TABLE site_type_packs ADD COLUMN IF NOT EXISTS subtitle_kind TEXT;
-- 存量行按既有口径回填（builtin 包按 0146 函数同款规则；custom 留空 = apply 时按 code 回落）
UPDATE site_type_packs SET subtitle_kind = 'lyric'
WHERE code IN ('music', 'lossless') AND subtitle_kind IS NULL;
UPDATE site_type_packs SET subtitle_kind = 'subtitle'
WHERE code NOT IN ('music', 'lossless') AND subtitle_kind IS NULL
  AND code <> 'general'; -- general 保持 NULL：作为「未表态」的缺省包

-- ============ 5) 显式字幕口径应用（custom 包不再按 code 猜） ============
CREATE OR REPLACE FUNCTION apply_subtitle_kind_explicit(is_lyric boolean) RETURNS void
LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO site_settings (name, value, descr) VALUES
    ('subtitle_kind', CASE WHEN is_lyric THEN 'lyric' ELSE 'subtitle' END,
     '字幕区口径：subtitle 影视字幕 / lyric 歌词'),
    ('subtitle_label', CASE WHEN is_lyric THEN '歌词' ELSE '字幕' END,
     '字幕区显示名（导航与页面标题）'),
    ('subtitle_ext_whitelist',
     CASE WHEN is_lyric THEN 'lrc,srt,ass,zip' ELSE 'srt,ass,ssa,sup,idx,sub,cue,zip,rar,7z' END,
     '允许上传的字幕扩展名（逗号分隔）'),
    ('subtitle_show_fps', CASE WHEN is_lyric THEN '0' ELSE '1' END,
     '是否显示 FPS 字段（歌词无需）')
  ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
END $$;
