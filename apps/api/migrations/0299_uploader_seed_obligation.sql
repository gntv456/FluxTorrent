-- 0299（假种/作弊审计 2026-10-07 二轮 P2-8，报告 _doc/假种与作弊漏洞深度审计-2026-10-07.md）
-- 发布者保种义务后台可配置：出种人数阈值（默认 3 人）。
--
-- 背景：站内此前无「发布者须保种」规则（「复活任务」是另一机制）。站长要求把
-- 发布者保种做成后台可配置，默认出种 3 人。落成一个开关 + 阈值：
--   uploader_seed_enabled = 'on'/'off'（默认 on 开启；用 enum 而非 bool——
--     设置面板无 checkbox 控件，bool 会退化成裸文本框，enum 才有下拉）
--   uploader_seed_min     = 出种人数阈值（默认 3，clamp 0..50）
-- worker 用它判定「发布者是否尽责」：发布者名下种子若当前做种人数低于阈值，
-- 视为发布者未尽责 → 该种子进入保种区等待他人认领补种（与既有 preserve_seed
-- 死种入保种区同一张表、同一套认领/结算流程，不新增机制）。
-- 全部幂等（ON CONFLICT DO NOTHING/UPDATE），重跑安全。

-- ① 开关（enum：面板渲染为下拉，不用 bool 文本框）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('uploader_seed_enabled', 'on', '发布者保种义务开关', 'main')
ON CONFLICT (name) DO NOTHING;

-- ② 阈值：默认 3 人（站长要求）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('uploader_seed_min', '3', '发布者保种出种人数阈值', 'main')
ON CONFLICT (name) DO NOTHING;

-- ③ 后台设置面板元数据
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step,
   options, group_key, card_order)
VALUES
  ('uploader_seed_enabled', 'enum', '发布者保种义务', 'Uploader reseed obligation',
   '开启后：发布者名下的种子，若当前做种人数低于「出种人数阈值」，视为发布者未尽责，'
   '自动进入保种区等待其他用户认领补种。关闭则不执行该判定。',
   NULL, NULL, NULL, NULL,
   '{"options":[{"label":"开启","value":"on"},{"label":"关闭","value":"off"}]}',
   '上传限制', 9),
  ('uploader_seed_min', 'number', '出种人数阈值', 'Minimum seeders',
   '发布者保种义务的出种人数阈值（默认 3 人）。发布者名下种子当前做种人数低于此值，'
   '即视为发布者未尽责、进入保种区。0 = 不看做种人数（仅按既有死种条件入区）。',
   '人', 0, 50, 1, NULL, '上传限制', 10)
ON CONFLICT (name) DO NOTHING;

-- ④ 已存在则补齐/修正元数据（可重复执行，修正历史缺失）
UPDATE settings_meta SET
  hint = '开启后：发布者名下的种子，若当前做种人数低于「出种人数阈值」，视为发布者未尽责，自动进入保种区等待其他用户认领补种。关闭则不执行该判定。',
  options = '{"options":[{"label":"开启","value":"on"},{"label":"关闭","value":"off"}]}'
WHERE name = 'uploader_seed_enabled' AND NOT readonly;

UPDATE settings_meta SET
  hint = '发布者保种义务的出种人数阈值（默认 3 人）。发布者名下种子当前做种人数低于此值，即视为发布者未尽责、进入保种区。0 = 不看做种人数（仅按既有死种条件入区）。'
WHERE name = 'uploader_seed_min' AND NOT readonly;
