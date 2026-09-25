-- 0197：邀请系统进模块注册表（四审 L5 P1 —— 29 键里从来没有 invites）
--
-- 现状：/api/v1/invites* 四个端点不在网关映射、页面无 requireModule、导航与
-- 用户菜单入口硬编码渲染、job:expire_invites 未挂模块锁 ⇒ 站长无法关掉邀请玩法。
-- 本迁移只补数据面；键本身在 modules.rs::key（INVITES）与网关/worker/前端守卫同步。
INSERT INTO modules (key, name_zh, name_en, descr, grp, is_on)
VALUES (
    'invites', '邀请系统', 'Invites',
    '邀请码生成/兑换与邀请配额；关闭后入口、页面与接口一并下线', 'community', true
) ON CONFLICT (key) DO NOTHING;

-- 11 个站型包的 modules 快照是完整键集（0178/0179 拉平过），补第 30 键时同步带上，
-- 否则 apply 之后该键回落 default_on 而包内快照不完整、diff 也会漏报。
-- 邀请是通用建站能力（任何站型都可能靠邀请冷启动），各包一律给 true。
UPDATE site_type_packs
SET modules = modules || '{"invites": true}'::jsonb
WHERE modules IS NOT NULL AND NOT (modules ? 'invites');
