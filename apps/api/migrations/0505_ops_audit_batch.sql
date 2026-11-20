-- 0505 运营体检批（渠道口+漏斗+圈选+参数化）：
-- ① shoutbox.user_id 放宽为可空：官方广播（POST /admin/shout-broadcast）
--    以系统身份落行（user_id=NULL），读侧 username 本就 LEFT JOIN 可空。
-- ② announcements.published_at 定时发布：旧行回填 created_at 保持行为
--    不变；读侧只出已到点的公告，写口可传未来时间预热。
-- ③ 生命周期/派奖参数化两键（onboarding day3 窗口 + arcade 周奖品类），
--    缺省值=既有硬编码值，应用后行为零漂移。

ALTER TABLE shoutbox ALTER COLUMN user_id DROP NOT NULL;

ALTER TABLE announcements
  ADD COLUMN IF NOT EXISTS published_at TIMESTAMPTZ;
UPDATE announcements SET published_at = created_at WHERE published_at IS NULL;
COMMENT ON COLUMN announcements.published_at IS
  '定时发布：到点后才在首页可见；NULL 视为立即';

-- site_settings 先行（settings_meta.name 有 FK 指向它）；grp 用既有
-- 枚举值（misc=杂项运营键；games_minclass 0490 先例开过 'games'）
INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('onboarding_day3_days', '3', '新手首下提醒窗口（天）', 'misc'),
  ('arcade_week_prize_kinds', 'lucky,fishing', '娱乐屋周奖品类', 'games')
ON CONFLICT (name) DO NOTHING;

-- 运营广播面板导航行（0504 promoops 先例同口径）
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '运营广播', '/admin?tool=opsbroadcast',
       '站内信/聊天室广播 + 注册 cohort 生命周期漏斗', 17, 'ops', 98,
       'opsbroadcast', 'settings.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'opsbroadcast'
);

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit,
                           min, max, step, min_class, group_key, card_order)
VALUES
  ('onboarding_day3_days', 'number', '新手首下提醒窗口',
   'Onboarding day-3 window',
   '注册满 N 天仍无完成下载时发提醒站内信（onboarding_followup job）；0 = 关闭该提醒',
   '天', 0, 60, 1, 99, '引导', 1),
  ('arcade_week_prize_kinds', 'text', '娱乐屋周奖品类',
   'Arcade weekly prize kinds',
   '参与每周 TOP3 派奖的游戏 kind，逗号分隔（如 lucky,fishing）；留空 = 全关',
   NULL, NULL, NULL, 1, 99, '娱乐屋', 1)
ON CONFLICT (name) DO NOTHING;
