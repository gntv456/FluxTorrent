-- 0110 settings_meta 补录（社交层/考核参数可配置）
--
-- 背景：settings_meta 是站点设定 UI 的字段目录（0039 建立）——**没有 meta 行的
-- site_settings 项不会出现在 /admin/settings 界面里**，只能改库。本次排查发现：
--   exam_onboard_days / social_* / jixiao_* 等 0093~0106 新功能写入的配置项
--   全部缺 meta 行 → 站长在后台看不到这些参数。
-- （module_* 开关已由 0107 模块注册表迁移统一登记，本迁移不重复。）
-- 注意：只补目录（INSERT ... DO NOTHING），不动 site_settings 现有值。

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class, visible) VALUES
  -- 社交层参数（0102/0103）：「社交」卡片
  ('social_endangered_seeders', 'number', '濒危判定做种数阈值', 'Endangered seeder threshold', '做种数 ≤ 该值的资源列入濒危雷达（默认 1）', '社交', 1, 99, true),
  ('social_health_seeders', 'number', '健康做种数阈值', 'Healthy seeder threshold', '做种数 > 该值视为健康（对齐保种区移出口径，默认 7）', '社交', 2, 99, true),
  ('social_team_default_days', 'number', '组队契约默认期限（天）', 'Default team contract days', '发起保种协作的默认截止天数（默认 14）', '社交', 3, 99, true),
  ('social_rep_on_fulfilled', 'number', '信誉：契约完成加分', 'Reputation: fulfilled delta', '组队契约达标时信誉加分（默认 20）', '社交', 4, 99, true),
  ('social_rep_on_failed', 'number', '信誉：到期未达标减分', 'Reputation: failed delta', '契约到期未达标的信誉减分（默认 -5，填负数）', '社交', 5, 99, true),
  ('social_rep_on_withdrawn', 'number', '信誉：中途退出减分', 'Reputation: withdrawn delta', '中途退出契约的信誉减分（默认 -15，填负数）', '社交', 6, 99, true),
  ('social_rep_min', 'number', '信誉下限', 'Reputation floor', '信誉分下限（默认 0）', '社交', 7, 99, true),
  ('social_rep_max', 'number', '信誉上限', 'Reputation ceiling', '信誉分上限（默认 2000）', '社交', 8, 99, true),
  -- 考核参数（0093/0106）：「考核」卡片
  ('exam_onboard_days', 'number', '新人考核派发窗口（天）', 'Onboard exam assign window', '注册后 N 天内自动派发新人转正考核（默认 30；是派发窗口不是考核期限）', '考核', 1, 99, true),
  ('jixiao_claim_window_days', 'number', '绩效补领窗口（天）', 'Jixiao claim window', '月末自动结算后允许手动补领的天数（默认 7）', '考核', 2, 99, true),
  ('jixiao_bonus_months_per_step', 'number', '绩效加成档位月数（全站默认）', 'Jixiao bonus months per step', '每 N 个达标月触发一档加成；岗位级配置优先（默认 3）', '考核', 3, 99, true),
  ('jixiao_bonus_percent_per_step', 'number', '绩效每档加成百分比（全站默认）', 'Jixiao bonus percent per step', '每档加成百分比；岗位级配置优先（默认 10）', '考核', 4, 99, true)
ON CONFLICT (name) DO NOTHING;
