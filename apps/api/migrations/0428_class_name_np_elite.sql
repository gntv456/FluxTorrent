-- 0428：成长线等级名统一回 NP 官方口径（补齐 0277 未完成的最后一级）
--
-- 现网实况（2026-10-28 直查 user_classes）：十级成长线里 id=3 仍是
-- 站长后台改过的「LV3」，其余 0/1/2/4-9 已是 NP 官方名（Peasant/User/
-- Power User/Crazy User/.../Nexus Master）。本迁移只把 3 号补成
-- Elite User——成长线归一，Peasant（降级地板之上的显赫档）与
-- VIP/职务级（20/91-99）一概不动。
--
-- 与 0277 的关系：0277 已种过全套官方名，本条是「后台改名漂移」的
-- 一次性纠偏；幂等（重复执行结果一致）。

UPDATE user_classes SET name = 'Elite User' WHERE id = 3 AND name <> 'Elite User';
