-- 0112: 通用建站系统 U5 §12.3 —— 反作弊配置面（Ratio Watch 参数已在 worker 读
-- site_settings，但无 settings_meta 登记，后台不可调；命中动作只有「暂停下载」一档）。
--
-- 本迁移：
-- 1) ratio_watch_threshold / ratio_watch_days 登记 settings_meta（防作弊分组）；
-- 2) 新增 ratio_watch_action 命中动作分级（warn=仅记录警告 / limit_download=暂停下载，默认后者=现状 T3）；
-- 3) agent 命中动作分级 agent_hit_action（log=仅记录 / warn=记录+警告信，默认 log=现状）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('ratio_watch_threshold', '0.4', 'Ratio Watch 分享率警戒线', 'anticheat'),
('ratio_watch_days', '14', 'Ratio Watch 观察期天数', 'anticheat'),
('ratio_watch_action', 'limit_download', '观察期到期处置：warn=仅警告 / limit_download=暂停下载', 'anticheat'),
('agent_hit_action', 'log', '客户端名单命中动作：log=仅记录 / warn=记录+警告信', 'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max, options, group_key, card_order) VALUES
('ratio_watch_threshold', 'number', '分享率警戒线', 'Ratio watch threshold', '', 0.01, 10, NULL, 'anticheat', 1),
('ratio_watch_days', 'number', '观察期天数', 'Ratio watch days', '天', 1, 90, NULL, 'anticheat', 2),
('ratio_watch_action', 'enum', '到期处置动作', 'Expiry action', NULL, NULL, NULL,
 '{"options": ["warn", "limit_download"]}'::jsonb, 'anticheat', 3),
('agent_hit_action', 'enum', '客户端名单命中动作', 'Agent hit action', NULL, NULL, NULL,
 '{"options": ["log", "warn"]}'::jsonb, 'anticheat', 4)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en, unit = EXCLUDED.unit,
      min = EXCLUDED.min, max = EXCLUDED.max, options = EXCLUDED.options,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;
