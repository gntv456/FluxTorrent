-- 0371_reg_sanction_settings_meta.sql：M3/M4 新设置键的后台编辑面。
-- settings_meta 决定设置页的字段类型/标签/分组/可写等级；缺行则键只在
-- 原始 settings 列表里以纯文本出现（无控件、无校验）。幂等，可重跑。
--
-- 分组口径：
--   anticheat    → 「防作弊」（M3 处置参数）
--   注册与邀请    → 与既有 maxusers 同组（M4 注册治理）

-- ① M3：作弊处置参数
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step,
   group_key, card_order, min_class)
VALUES
  ('cheat_ban_default_days', 'number', '作弊停权默认天数',
   'Default cheat suspension (days)',
   'resolve 下达 ban 时未指定天数则用它；0 或空按 30 处理',
   '天', 1, 3650, 1, 'anticheat', 30, 99),
  ('appeal_token_days', 'number', '申诉令牌有效期',
   'Appeal token validity (days)',
   '封禁通知里一次性申诉令牌的有效期，过期作废（用后即焚）',
   '天', 1, 365, 1, 'anticheat', 31, 99)
ON CONFLICT (name) DO UPDATE SET
  type = EXCLUDED.type, label_zh = EXCLUDED.label_zh,
  label_en = EXCLUDED.label_en, hint = EXCLUDED.hint, unit = EXCLUDED.unit,
  min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
  group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order,
  min_class = EXCLUDED.min_class, updated_at = now();

-- ② M4：注册治理参数
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step,
   options, group_key, card_order, min_class)
VALUES
  ('reg_window', 'textarea', '注册窗口（JSON）',
   'Registration window (JSON)',
   '空 = 不限制。格式：{"open":"2026-01-01T00:00:00Z","close":"2026-01-07T00:00:00Z","slots":200,"label":"元旦开放"}；窗口外或名额满拒绝注册',
   NULL, NULL, NULL, NULL, NULL, '注册与邀请', 13, 99),
  ('reg_ip_limit', 'number', '同 IP 注册上限',
   'Registrations per IP',
   '计数窗口内同一网络环境最多注册数（0 = 不限制）。IP 只存 sha3 摘要',
   '个', 0, 100, 1, NULL, '注册与邀请', 14, 99),
  ('reg_ip_window_hours', 'number', '同 IP 计数窗口',
   'Per-IP counting window',
   '同 IP 注册计数的回看窗口（小时）', '小时', 1, 8760, 1, NULL,
   '注册与邀请', 15, 99),
  ('reg_ip_retention_days', 'number', '注册摘要保留天数',
   'Registration digest retention',
   'reg_ip_signups 摘要保留期，到期由 worker 每日清理（仅存 sha3，无明文 IP）',
   '天', 1, 3650, 1, NULL, '注册与邀请', 16, 99),
  ('reg_block_disposable', 'yesno', '拒绝一次性邮箱',
   'Block disposable email',
   '开启后，域名命中「一次性邮箱域名库」的注册被拒',
   NULL, NULL, NULL, NULL, NULL, '注册与邀请', 17, 99)
ON CONFLICT (name) DO UPDATE SET
  type = EXCLUDED.type, label_zh = EXCLUDED.label_zh,
  label_en = EXCLUDED.label_en, hint = EXCLUDED.hint, unit = EXCLUDED.unit,
  min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
  options = EXCLUDED.options, group_key = EXCLUDED.group_key,
  card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class,
  updated_at = now();

-- ③ maxusers 补标签（既有键：type/label 已在，补英文与提示）
UPDATE settings_meta SET
  label_en = 'Max users', unit = '个',
  hint = '站点最大用户数（0 = 不限制）。达到上限后注册通道关闭，含邀请通道',
  updated_at = now()
WHERE name = 'maxusers';

-- ④ 旧式站点设置列表的分组（site_settings.grp）
UPDATE site_settings SET grp = 'anticheat'
WHERE name IN ('cheat_ban_default_days', 'appeal_token_days')
  AND (grp IS NULL OR grp <> 'anticheat');
UPDATE site_settings SET grp = '注册与邀请'
WHERE name IN ('maxusers', 'reg_window', 'reg_ip_limit',
               'reg_ip_window_hours', 'reg_ip_retention_days',
               'reg_block_disposable')
  AND (grp IS NULL OR grp <> '注册与邀请');
