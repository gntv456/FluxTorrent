-- 0226_onboarding_presets.sql — C4 新手运营双模板（竞品对比行动批）
--
-- 背景：两大竞品代表两种新手哲学（源码级调研结论）——
--   · NexusPHP「考核淘汰制」：入职考核 + H&R 从严 + 低保户降级恐惧，高压高留存；
--   · UNIT3D「缓冲宽进制」：初始上传缓冲 + H&R 宽限预警 + 无考核，宽进宽养。
-- 定位要求「不偏向任何 PT 类型」→ 运营风格也应可选。本迁移只落「选择态 + 参数簇」，
-- 不改任何既有用户数据；两套参数的语义全部映射到既有消费点：
--   hr_hours / hr_violation_limit / hr_warn（worker hr.rs 消费）
--   exam 模块（tasks.kind='onboard' + auto_assign 派发）
--   class_rules.demotable（等级自动升降）
--   initial_upload_gb（注册初始上传量；无既有键则本迁移建立，register 消费）

-- 1) 选择态设定键（none | strict | lenient；默认 none = 维持现状）
INSERT INTO site_settings (name, value, grp)
VALUES ('onboarding_preset', 'none', 'ops')
ON CONFLICT (name) DO UPDATE SET updated_at = now();

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options, group_key, card_order)
VALUES ('onboarding_preset', 'enum', '新手运营模板', 'Onboarding preset',
        'none=维持现状（缺省）；strict=考核淘汰制（入职考核+H&R 从严+低保户降级）；lenient=缓冲宽进制（初始上传缓冲+H&R 宽限+无考核）。切换只写参数，不动用户数据。',
        '{"options":[{"value":"none","label":"维持现状"},{"value":"strict","label":"考核淘汰制"},{"value":"lenient","label":"缓冲宽进制"}]}'::jsonb,
        'ops', 1)
ON CONFLICT (name) DO NOTHING;

-- 2) 参数簇载体（初值空 = 未启用；应用模板时由后端写入具体值）
INSERT INTO site_settings (name, value, grp)
VALUES ('initial_upload_gb', '0', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max, step, group_key, card_order)
VALUES ('initial_upload_gb', 'number', '新用户初始上传量', 'Initial upload (GB)',
        '注册即得的缓冲上传量（宽进制核心参数；0=不给）', 'GB', 0, 10240, 1, 'main', 30)
ON CONFLICT (name) DO NOTHING;

-- 3) H&R 预警（宽制制的“prewarn”）：到期前 N 小时发提醒私信；0=关
INSERT INTO site_settings (name, value, grp)
VALUES ('hr_prewarn_hours', '0', 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max, step, group_key, card_order)
VALUES ('hr_prewarn_hours', 'number', 'H&R 到期预警提前量', 'HR prewarn lead hours',
        'H&R 考察期结束前 N 小时发站内提醒（0=关闭；宽进制模板设 24）', '小时', 0, 720, 1, 'torrent', 6)
ON CONFLICT (name) DO NOTHING;

-- 4) 一次性邮箱域名黑名单（P2 触点 #4）
--    复用既有 email_bans（0032）的「@domain」形态即可整域封禁，且注册链路
--    （register.rs email_banned 查询）已消费该形态——无需新表，只种常见一次性
--    域名并登记后台入口说明。allow 行可豁免（与既有语义一致）。
INSERT INTO email_bans (pattern, mode, note)
SELECT d, 'ban', '一次性邮箱域名（0226 预置，站长可在后台邮箱黑名单增删）'
FROM unnest(ARRAY[
  '@mailinator.com', '@10minutemail.com', '@guerrillamail.com',
  '@yopmail.com', '@temp-mail.org', '@throwawaymail.com',
  '@sharklasers.com', '@getnada.com', '@dispostable.com', '@trashmail.com'
]) AS d
ON CONFLICT (pattern) DO NOTHING;

-- 5) 自助解封（P2 触点 #24）：冷却期内一次
ALTER TABLE users ADD COLUMN IF NOT EXISTS self_unban_used BOOLEAN NOT NULL DEFAULT FALSE;

INSERT INTO site_settings (name, value, grp)
VALUES ('self_unban_cooldown_hours', '0', 'ops')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max, step, group_key, card_order)
VALUES ('self_unban_cooldown_hours', 'number', '自助解封冷却期', 'Self-unban cooldown',
        '被禁用户自助解封（首次宽恕）后再次自助的冷却小时数；0=关闭自助解封。用掉一次记 self_unban_used。', '小时', 0, 8760, 1, 'ops', 2)
ON CONFLICT (name) DO NOTHING;

-- 6) 经济购买上限阀门（C5）：用户缓冲量（上传-下载）超过该值禁购上传量类商品；0=不限制
INSERT INTO site_settings (name, value, grp)
VALUES ('economy_max_buffer_gb', '0', 'economy')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max, step, group_key, card_order)
VALUES ('economy_max_buffer_gb', 'number', '上传量商品购买上限（缓冲量）', 'Upload purchase buffer cap',
        '反通胀阀门：用户缓冲量（上传-下载）超过该 GB 值后禁止再购买上传量类商品；0=不限制（缺省）', 'GB', 0, 102400, 1, 'economy', 8)
ON CONFLICT (name) DO NOTHING;

-- 7) 彩虹 ID / 用户名染色 SKU（P2 触点 #17）：效果链路（kind + user_dressups +
--    dressup wear + 前端 rainbow 样式）0207 已就位，此处只上架 SKU（永久单档，
--    与 avatar_frame 同形；时效档留待 dressup 表加 expires_at 后再上）。
INSERT INTO shop_items (name, kind, price, config, active)
SELECT '彩虹用户名（永久）', 'rainbow_name', 8000, '{}'::jsonb, true
WHERE NOT EXISTS (SELECT 1 FROM shop_items WHERE kind = 'rainbow_name' AND active);

-- 8) staff_panel 挂「新手模板」「经济仪表」入口（ops 分区；sysop 可见；列形态同 0054 先例）
INSERT INTO staff_panel_entries (panel, name, url, info, sort, section, min_class, tab_key) VALUES
('sysop', '新手运营模板', '/admin?tool=onboarding', '一键切换考核淘汰制/缓冲宽进制参数簇', 61, 'ops', 99, 'onboarding'),
('sysop', '经济仪表', '/admin?tool=economy', '火花产出/消耗周曲线、池子、Top SKU、反通胀阀门', 62, 'ops', 99, 'economy')
ON CONFLICT DO NOTHING;
