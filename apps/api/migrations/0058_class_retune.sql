-- 等级微调：用户层名称前移（去掉「幼树」）；维护开发员由 90 提升至 98
--
-- ⚠ 陷阱：users.class_id 有外键 users_class_id_fkey → user_classes.id，
--   因此**不能 DELETE + INSERT** 重建等级行（有用户引用即报错）。
--   改法：对用户层用 UPDATE 改名（行数不变，名字整体前移一位），
--         管理职级先重映射用户再删旧行。
--
-- 用户层新序列（1-12）：新芽 幼苗 小苗 小树 乔木 大树 花蕾 开花 结果 硕果 森林 生态
-- 锚点不变：大树(6)=1TB/50/720h/90d、开花(8)=4TB/120/1440h/180d、硕果(10)=10TB/300/2880h/365d
-- 注：id 结构不变意味着原 6 级用户的等级名会前移，其门槛随之提高，
--     不满足者由 worker class_auto_adjust 自动降级修正——这是预期行为。

-- ============ 1) 用户层改名（0-4 不变，5-12 依次前移并补「生态」） ============
UPDATE user_classes SET name = '乔木', pod_level = 5  WHERE id = 5;
UPDATE user_classes SET name = '大树', pod_level = 6  WHERE id = 6;
UPDATE user_classes SET name = '花蕾', pod_level = 7  WHERE id = 7;
UPDATE user_classes SET name = '开花', pod_level = 8  WHERE id = 8;
UPDATE user_classes SET name = '结果', pod_level = 9  WHERE id = 9;
UPDATE user_classes SET name = '硕果', pod_level = 10 WHERE id = 10;
UPDATE user_classes SET name = '森林', pod_level = 11 WHERE id = 11;
UPDATE user_classes SET name = '生态', pod_level = 12 WHERE id = 12;

-- ============ 2) 升级门槛（12 条，与名称同步） ============
DELETE FROM class_rules WHERE class_id BETWEEN 1 AND 13;
INSERT INTO class_rules (class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days, demotable) VALUES
  (1,  'LV1 新芽',  0,              0,   0,    0,   false),
  (2,  'LV2 幼苗',  10737418240,    2,   24,   3,   true),
  (3,  'LV3 小苗',  53687091200,    5,   72,   7,   true),
  (4,  'LV4 小树',  128849018880,   10,  144,  15,  true),
  (5,  'LV5 乔木',  536870912000,   35,  420,  55,  true),
  (6,  'LV6 大树',  1099511627776,  50,  720,  90,  true),
  (7,  'LV7 花蕾',  2199023255552,  80,  1080, 130, true),
  (8,  'LV8 开花',  4398046511104,  120, 1440, 180, true),
  (9,  'LV9 结果',  7696581394432,  200, 2160, 260, true),
  (10, 'LV10 硕果', 10995116277760, 300, 2880, 365, true),
  (11, 'LV11 森林', 19791209299968, 450, 4320, 540, true),
  (12, 'LV12 生态', 32985348833280, 600, 6480, 730, true);

-- ============ 3) 维护开发员 90 → 98（先解除用户引用，再删旧行） ============
UPDATE users SET class_id = 98 WHERE class_id = 90;
INSERT INTO user_classes (id, name, pod_level, min_uploaded, min_ratio, min_age_days, privileges) VALUES
  (98, '维护开发员', 98, 0, 0, 0, '{}'::jsonb)
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, pod_level = EXCLUDED.pod_level;
-- 仅当确认无引用时删除 90（避免再次触发外键失败）
DELETE FROM user_classes WHERE id = 90
  AND NOT EXISTS (SELECT 1 FROM users u WHERE u.class_id = 90);

-- 其余管理职级确保就位
INSERT INTO user_classes (id, name, pod_level, min_uploaded, min_ratio, min_age_days, privileges) VALUES
  (91, '发布员',   91, 0, 0, 0, '{}'::jsonb),
  (92, '论坛版主', 92, 0, 0, 0, '{}'::jsonb),
  (93, '总版主',   93, 0, 0, 0, '{}'::jsonb),
  (94, '管理员',   94, 0, 0, 0, '{}'::jsonb),
  (95, '主管',     95, 0, 0, 0, '{}'::jsonb),
  (99, '站长',     99, 0, 0, 0, '{}'::jsonb)
ON CONFLICT (id) DO NOTHING;

-- ============ 4) 权限档位：保留 90（管理组门槛），新增 98（维护开发员及以上） ============
-- 98 档 = 系统运维能力：比管理员(94/95 → 落在 93 档) 多出底层运维权限，
-- 但不含 99 档的用户管理高危操作（改等级 / 删用户 / 重置密码 等）
INSERT INTO role_permissions (role_type, role_key, permission_key)
SELECT 'class', '98', key FROM permissions WHERE key IN (
  'syslog.view','dbstats.view','cleanup.run','maxlogin.view',
  'sitepacks.manage','locations.manage','plugins.manage','clearcache','bans.manage'
)
ON CONFLICT DO NOTHING;
