-- 0497 渠道批：mail_outbox 单用户频控键（产品体检 2026-11-18 A2）。
--
-- mail_outbox 只有 dedupe_key 幂等，无每用户频率上限。召回/促销批量入队
-- 一旦放开，单用户可能被连续多封灌爆（防骚扰底线）。发送侧（worker
-- mail_outbox_drain）按 user_id 24h 窗口计数，超上限的行推迟（不丢、
-- 不计失败重试）。上限键化可热调，0=不限。
--
-- 同批口径记录（无 DDL）：
-- · massmail 圈选（A4）在 /admin/massmail 请求体加 audience 字段，
--   SQL WHERE 段实现（users 既有列），不建表。
-- · 评分来源读侧（A5）：work_meta.source（0399）已存在且 works_sql
--   META_SEL 已下发 source，前端补徽标即可，无迁移动作。

INSERT INTO site_settings (name, value, grp) VALUES
  ('mail_user_daily_cap', '5', 'mail')
ON CONFLICT (name) DO NOTHING;

-- settings_meta 登记（后台「邮件」区可调）
INSERT INTO settings_meta (name, type, label_zh, label_en, hint,
                           group_key, card_order) VALUES
  ('mail_user_daily_cap', 'number', '单用户邮件日上限',
   'Per-user mail daily cap',
   'mail_outbox 24h 窗口内单用户最多投递封数，超出推迟投递；0=不限',
   'mail', 7)
ON CONFLICT (name) DO NOTHING;
