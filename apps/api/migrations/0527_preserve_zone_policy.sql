-- 0527 保种区新口径（2026-11-27 站长定稿）：保种区 = 发布 ≥3 天 + 做种 1-4 人
-- 的温带种，区内一律免费；做种 >4 人自动移出并保持免费 3 天宽限后恢复原价。
-- 死种（0 做种）不再入保种区——由拯救区（/resurrections）+ 濒危预警承接。
--
-- 四件：① 出区上限键 preserve_seeders_max（worker 入区/出区同键同缺省）
--       ② 在区免费开关 preserve_free_enabled（preserve_free_promo 每小时对齐）
--       ③ preserve_dead_days 7 → 3（语义同步改为「发布到入区的最短天数」）
--       ④ 存量清洗：现有在区行里做种 0 或 >4 的按新口径移出（exited_at=now(),
--          reason=new_policy_sweep；0 做种行转拯救区候选，不做数据搬迁——
--          resurrections 本来就按 torrents 实时谓词取数）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('preserve_seeders_max', '4', '保种区做种人数上限（超过即自动移出保种区）', 'main'),
('preserve_free_enabled', 'on', '保种区种子免费（在区自动挂 free 促销，出区撤）', 'main')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max, step, group_key, card_order) VALUES
('preserve_seeders_max', 'number', '保种区做种上限', 'Preserve seeders cap',
 '保种区种子当前做种人数超过该值即自动移出，免费保持 3 天后恢复原价；入区条件同步要求做种人数不高于此值（默认 4）',
 '人', 1, 50, 1, '上传限制', 11),
('preserve_free_enabled', 'enum', '保种区免费', 'Preserve zone free',
 '开启后保种区在区种子自动全部免费（free 促销每小时对齐）；移出保种区后再免费 3 天，随后恢复原价',
 NULL, NULL, NULL, NULL,
 '{"options":[{"label":"开启","value":"on"},{"label":"关闭","value":"off"}]}',
 '上传限制', 12)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, unit = EXCLUDED.unit,
    min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
    group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- 入区天数口径更新：不再是「死种等待天数」，而是「发布到入区的最短天数」
UPDATE site_settings SET value = '3',
  descr = '种子发布满该天数且做种人数在 1..preserve_seeders_max 区间时进入保种区'
WHERE name = 'preserve_dead_days';

UPDATE settings_meta SET
  label_zh = '保种区入区天数', label_en = 'Preserve entry days',
  hint = '种子发布满该天数且当前做种人数在 1..「保种区做种上限」区间（无人做种的死种不进保种区，由拯救区承接）即自动进入保种区',
  unit = '天', min = 1, max = 365, step = 1,
  group_key = '上传限制', card_order = 10
WHERE name = 'preserve_dead_days';

-- 存量清洗：把不符合新口径的在区行按「策略切换移出」结转。
-- exited_at 落 now() ⇒ 列表 active 档立即消失；宽限（preserve_grace）由
-- worker 下一轮 preserve_exit 只对「本次 seeders 越限」行发放，本批行
-- 走 preserve_free 促销撤销即恢复原价（站长口径：策略切换不给宽限）。
UPDATE seed_preserve sp
SET exited_at = now(), exit_reason = 'new_policy_sweep'
FROM torrents t
WHERE t.id = sp.torrent_id
  AND sp.exited_at IS NULL
  AND (t.seeders = 0 OR t.seeders > 4);

-- 本批移出行若有在区的 preserve_free 短券，立即失效（不等 hourly 对齐）
UPDATE promotions p SET ends_at = now()
FROM seed_preserve sp
WHERE p.torrent_id = sp.torrent_id AND p.source = 'preserve_free'
  AND p.ends_at > now()
  AND sp.exit_reason = 'new_policy_sweep' AND sp.exited_at IS NOT NULL;

-- ⑤ 退役「发布者保种义务」两键：新口径下保种区入区只看
--    「发布天数 + 做种人数区间」，uploader_seed_enabled/min 失去消费者
--    （fake_switch 门禁：后台可写但全仓无人读 = 假开关）。
--    site_settings 行保留只摘 settings_meta（后台面板不再出现）。
DELETE FROM settings_meta WHERE name = 'uploader_seed_enabled';
DELETE FROM settings_meta WHERE name = 'uploader_seed_min';

