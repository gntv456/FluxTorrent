-- 0172: 维护开发员等级上移（用户反馈 2026-09-24）
--
-- 需求：维护开发员是技术岗位，权限应仅比站长（99）低一级 → 98。
-- 原 90 是 staff 门槛线（全站大量 `class_id >= 90` 判 staff、`< 90` 判普通用户），
-- 挪走后 90 空缺不影响门槛语义（最小 staff id 变为 91，>= 90 判断仍涵盖）。
--
-- 权限面无破坏（0054 体系）：
--  * role_permissions(role_type='class') 是累进语义（class_id >= role_key），
--    98 >= 90 原有权限全保留；
--  * roles/user_roles 职务与等级解耦，不受影响。
--
-- 有 FK（users.class_id → user_classes.id）：先挪用户再挪等级行。

UPDATE users SET class_id = 98 WHERE class_id = 90;
UPDATE user_classes SET id = 98 WHERE id = 90;
