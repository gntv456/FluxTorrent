-- 0107: 通用建站系统 U1 —— 模块注册表全量落地（策划案 §4）
--
-- 目标：把「可选功能域」收敛为三处同步的注册表（SQL 本表 / Rust modules.rs 常量 /
-- packages/domain-types ModuleKey），让 module_* 成为**行为开关**（T2）：
-- 导航、页面、API、worker 四处一致；默认值 = 当前教育站形态（T3，存量站零感知）。
--
-- 已有 4 键：module_textbooks / module_showcase / module_social / module_promo_buy（分散在各迁移）。
-- 本迁移补 20 个新键并统一登记。核心层（认证/种子/计费/促销/经济总账/搜索/i18n）不设开关（§4.1）。

-- ============ 1) 模块注册表 ============

CREATE TABLE IF NOT EXISTS modules (
    key         TEXT PRIMARY KEY,           -- 如 games / farm / bank（不带 module_ 前缀，site_settings 侧带前缀）
    name_zh     TEXT NOT NULL,
    name_en     TEXT NOT NULL,
    descr       TEXT NOT NULL DEFAULT '',   -- 影响范围说明（后台开关卡片提示）
    grp         TEXT NOT NULL DEFAULT 'community',  -- community / economy / fun / ops
    is_on       BOOLEAN NOT NULL DEFAULT TRUE       -- 默认值（T3：全部 = 现状）
);

INSERT INTO modules (key, name_zh, name_en, descr, grp, is_on) VALUES
-- 现有 4 键（登记进注册表，默认值沿用既有迁移）
('textbooks', '课本中心', 'Textbooks', '课本库浏览与种子关联（教育站型特有）', 'community', TRUE),
('showcase',  '展示区',   'Showcase',  '首页精选展示区块', 'community', FALSE),
('social',    '社交层',   'Social Layer', '濒危预警 / 组队契约 / 赛季', 'community', FALSE),
('promo_buy', '用户自购促销', 'Promo Purchase', '用户自购置顶/限时免费', 'economy', TRUE),
-- 娱乐（games 与 farm/gomoku/contests 平级兄弟，非父子 —— 允许「关农场留娱乐屋」）
('games',     '娱乐屋',   'Games',     '刮刮乐 / 猜大小 / 九宫格 / 趣味投票（合规敏感：机会类玩法）', 'fun', TRUE),
('farm',      '农场',     'Farm',      '好学农场种植/浇水/收获与市场价窗口', 'fun', TRUE),
('gomoku',    '五子棋',   'Gomoku',    '联机五子棋对局', 'fun', TRUE),
('contests',  '竞赛',     'Contests',  '周期竞赛报名与榜单', 'fun', FALSE),
-- 经济
('bank',      '银行',     'Bank',      '活期/定期/贷款与到期结息', 'economy', TRUE),
('shop',      '商店',     'Shop',      '火花商品购买', 'economy', TRUE),
('magic_pool','魔法值池', 'Magic Pool','捐赠入池/进度/排行（捐赠通道另见支付设置）', 'economy', TRUE),
('vouchers',  '代金券',   'Vouchers',  '代金券领取与使用', 'economy', TRUE),
('resurrections', '复活券', 'Resurrections', '删号复活申请', 'economy', TRUE),
('wishlist',  '愿望单',   'Wishlist',  '种子愿望单', 'economy', TRUE)
ON CONFLICT (key) DO NOTHING;

INSERT INTO modules (key, name_zh, name_en, descr, grp, is_on) VALUES
-- 社区
('forums',    '论坛',     'Forums',    '版块/主题/回帖/搜索', 'community', TRUE),
('messages',  '短讯',     'Messages',  '收发件箱/自定义信箱', 'community', TRUE),
('friends',   '好友',     'Friends',   '好友列表与操作', 'community', TRUE),
('offers',    '候选',     'Offers',    '候选投票与转正官种', 'community', TRUE),
('requests',  '求种',     'Requests',  '求种悬赏冻结与转移', 'community', TRUE),
('subtitles', '字幕',     'Subtitles', '字幕上传/下载/火花奖励', 'community', TRUE),
('preserve',  '保种区',   'Preserve',  '待保种列表与认领', 'community', TRUE),
('shoutbox',  '吐槽箱',   'Shoutbox',  '首页吐槽箱', 'community', TRUE),
-- 成长与运营
('attendance','签到',     'Attendance','每日签到/连签里程碑/补签', 'ops', TRUE),
('medals',    '勋章',     'Medals',    '勋章图鉴/购买/赠送/佩戴', 'ops', TRUE),
('dressup',   '装扮中心', 'Dressup',   '头像框等装扮', 'ops', TRUE),
('jixiao',    '绩效考核', 'Jixiao',    '指标自动采集与绩效领取', 'ops', TRUE),
('tasks',     '任务中心', 'Tasks',     '任务认领/限时任务/结算', 'ops', TRUE),
('exams',     '新人考核', 'Exams',     '新人考核与豁免', 'ops', TRUE),
('push',      'Web 推送', 'Web Push',  '浏览器推送订阅', 'ops', TRUE)
ON CONFLICT (key) DO NOTHING;

-- ============ 2) site_settings 落键（T3：默认 = 现状） ============
-- 既有 4 键不动（module_textbooks=yes / module_showcase=no / module_social=no / module_promo_buy=yes）。
-- 新 20 键全部 yes（= 教育站形态）。contests 特殊：当前竞赛页在线但属可选玩法，按注册表默认 FALSE 落键，
-- 让「开关关闭」路径从第一天就被真实使用（fail-close 验证）。push 落 yes。

INSERT INTO site_settings (name, value, descr, grp)
SELECT 'module_' || key,
       CASE WHEN is_on THEN 'yes' ELSE 'no' END,
       name_zh || '（模块开关）',
       'module'
FROM modules
WHERE key NOT IN ('textbooks', 'showcase', 'social', 'promo_buy', 'contests')
ON CONFLICT (name) DO NOTHING;

-- contests 显式落 no（新站默认关；存量教育站如已用竞赛需手动开，U2 走查时核对）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('module_contests', 'no', '竞赛（模块开关）', 'module')
ON CONFLICT (name) DO NOTHING;

-- ============ 3) settings_meta 登记（后台设置中心「模块开关」卡片，沿用 0039 机制） ============

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order)
SELECT 'module_' || key, 'yesno', name_zh || '（' || descr || '）', name_en,
       'module_' || grp, 10 + (row_number() OVER (ORDER BY grp, key))::int
FROM modules
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- ============ 4) menu_items 挂模块归属（§6.2：自定义菜单项随开关消失） ============

ALTER TABLE menu_items ADD COLUMN IF NOT EXISTS module_key TEXT REFERENCES modules(key) ON DELETE SET NULL;

-- ============ 5) 站型包 modules 默认值矩阵（§7.2 定稿播种；apply 后站长可任意改 = T1） ============
-- 写法：jsonb_set 链式合并，未提到的键保持包内现状。关闭列表来自策划案 §7.2。

UPDATE site_type_packs SET modules = modules::jsonb || '{"textbooks":false,"farm":false,"gomoku":false,"jixiao":false,"exams":false,"showcase":true}'::jsonb WHERE code = 'movie';
UPDATE site_type_packs SET modules = modules::jsonb || '{"textbooks":false,"farm":false,"gomoku":false,"jixiao":false,"subtitles":true}'::jsonb WHERE code = 'music';
UPDATE site_type_packs SET modules = modules::jsonb || '{"textbooks":false,"farm":false,"gomoku":false,"jixiao":false,"subtitles":true,"losslessPlayer":true}'::jsonb WHERE code = 'lossless';
UPDATE site_type_packs SET modules = modules::jsonb || '{"textbooks":false,"jixiao":false,"exams":false}'::jsonb WHERE code = 'anime';
UPDATE site_type_packs SET modules = modules::jsonb || '{"farm":false,"gomoku":false,"contests":false,"textbooks":true}'::jsonb WHERE code = 'ebook';
UPDATE site_type_packs SET modules = modules::jsonb || '{"textbooks":false,"farm":false,"contests":true}'::jsonb WHERE code = 'sports';
UPDATE site_type_packs SET modules = modules::jsonb || '{"textbooks":false,"jixiao":false,"exams":false,"contests":true}'::jsonb WHERE code = 'game';
UPDATE site_type_packs SET modules = modules::jsonb || '{"farm":false,"gomoku":false,"contests":false,"jixiao":false}'::jsonb WHERE code = 'software';
UPDATE site_type_packs SET modules = modules::jsonb || '{"textbooks":false,"farm":false,"gomoku":false,"jixiao":false,"exams":false,"showcase":true}'::jsonb WHERE code = 'documentary';
-- education：仅关 contests；general：全开基准（不动）
UPDATE site_type_packs SET modules = modules::jsonb || '{"contests":false}'::jsonb WHERE code = 'education';

-- ============ 7) 注册模式开关（§11.5）============

INSERT INTO site_settings (name, value, descr, grp) VALUES
('registration_mode', 'invite_only', '注册模式：invite_only / open / email_verify', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, options, group_key, card_order) VALUES
('registration_mode', 'enum', '注册模式', 'Registration Mode',
 '{"options": ["invite_only", "open", "email_verify"]}'::jsonb, 'basic', 90)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      options = EXCLUDED.options, group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- ============ 8) 契约自检（迁移内断言：modules 数量 = 29，settings 落键齐全） ============
-- 迁移失败即 CI 红灯，防三处漂移的第一道闸（第二道在 Rust/TS 契约测试）。
-- 注：注册表实际 29 键（初稿断言写 24 漏数了 5 个社区/运营键）。
DO $$
DECLARE n int;
BEGIN
    SELECT count(*) INTO n FROM modules;
    IF n <> 29 THEN
        RAISE EXCEPTION 'modules 注册表应有 29 键，实际 %', n;
    END IF;
    SELECT count(*) INTO n FROM site_settings WHERE name LIKE 'module_%';
    IF n < 29 THEN
        RAISE EXCEPTION 'site_settings.module_* 应至少 29 键（历史键含 promo_buy/social 等），实际 %', n;
    END IF;
END $$;
