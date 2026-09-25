-- 0204：勋章佩戴/赠送与邀请配置批（用户反馈 2026-09-25 #2/#3/#4）
--
-- 1) medals_max_worn：站长可设「用户最多同时佩戴几枚勋章」（缺省 3）。
--    佩戴 API 从单佩戴位改多佩戴位（上限内自由多戴），展示位 LIMIT 跟随此键。
-- 2) medals.gift_fee_bp：per-勋章赠送手续费（基点，NULL = 回退全站 gift_tax_bp）。
--    赠送语义本就是代购式（送方付费），未拥有也可送——本次放开前端 gating。
-- 3) 邀请系统配置化：周配额 / TTL 硬编码迁入 site_settings（bonus 组），
--    站长可调；同时为 invites_weekly_class3 等四键补 settings_meta。

-- ===== 1) 佩戴上限 =====
INSERT INTO site_settings (name, value, descr, grp) VALUES
('medals_max_worn', '3', '用户最多同时佩戴勋章数', 'bonus')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, unit, min, max, step, group_key,
     card_order)
VALUES
('medals_max_worn', 'number', '最多同时佩戴勋章', 'Max worn medals',
 '用户可同时佩戴的勋章数量上限；各展示位（弹窗/楼层/主页）按此上限展示',
 '个', 1, 12, 1, '勋章', 10)
ON CONFLICT (name) DO NOTHING;

-- ===== 2) per-勋章赠送手续费 =====
ALTER TABLE medals ADD COLUMN IF NOT EXISTS gift_fee_bp integer;

COMMENT ON COLUMN medals.gift_fee_bp IS
    '赠送手续费（基点）；NULL = 回退全站 gift_tax_bp（缺省 500 = 5%）';

-- ===== 3) 邀请配置四键 =====
INSERT INTO site_settings (name, value, descr, grp) VALUES
('invites_weekly_class3', '2', 'LV3+ 每周邀请配额', 'bonus'),
('invites_weekly_bonus', '4', '外联员/VIP 每周邀请配额', 'bonus'),
('invite_ttl_hours', '72', '邀请码有效期（小时）', 'bonus'),
('invite_admin_ttl_days', '30', '管理端直发邀请码有效期（天）', 'bonus')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, unit, min, max, step, group_key,
     card_order)
VALUES
('invites_weekly_class3', 'number', 'LV3+ 每周邀请配额', 'Weekly invites (LV3+)',
 '等级 ≥3 用户的每周可生成邀请码数量', '枚', 0, 100, 1, '邀请', 10),
('invites_weekly_bonus', 'number', '外联/VIP 每周邀请配额',
 'Weekly invites (invites.bonus)', '持 invites.bonus 权限用户的每周配额', '枚',
 0, 100, 1, '邀请', 11),
('invite_ttl_hours', 'number', '邀请码有效期', 'Invite TTL',
 '用户生成邀请码的有效时长', '小时', 1, 720, 1, '邀请', 12),
('invite_admin_ttl_days', 'number', '管理直发邀请码有效期',
 'Admin-issued invite TTL', '管理端直发邀请码的有效时长', '天', 1, 365, 1,
 '邀请', 13)
ON CONFLICT (name) DO NOTHING;
