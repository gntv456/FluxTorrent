-- 0361：新增 cheaters.view（批六十一 P-231，2026-10-13 职员读口实测）
--
-- 现网事实（闸门 scripts/pt_audit_ca_staff_view.py 的 CA0b/CA5 读数）：
--   role_permissions 里 staff.panel 由 class/90 授予，而 `authz.rs:96-98` 对
--   class 键的语义是「user_classes.id >= role_key」的**阈值**（库里并没有
--   id=90 这一档）⇒ 91 发布员、92 论坛版主、93 总版主、94 管理员全都持有
--   staff.panel。而 GET /admin/cheaters 过去只判 staff()，于是这份
--   「疑似作弊会话」名单（含每人近 1 小时上传增量与折算速度、种子名、
--   user_id）对最低档职员也可读——它是管理组据以处置的调查材料。
--
-- 口径照 0285：passkey 明文（user.passkey.reveal）当时就是「主管(95) 与
-- 站长(99) 持有；版主/管理员不可（默认最小权限）」。class 是阈值语义，
-- 所以一行 class/95 就覆盖 95/98/99。矩阵读口（P-232）不加新键，改为与
-- 写口同档 settings.manage（现网仅 class/99）。
--
-- 全部幂等；新键 implemented=true（代码侧已接线：admin_http/site.rs 引用
-- perm::CHEATERS_VIEW，CA4 三件齐判据会盯住「加了键没人用」）。

INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('cheaters.view', '查看作弊者探测名单', 'system',
 '读取 snatches 实时速度与短窗上传增量超阈值的可疑会话名单', 112, true)
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_type, role_key, permission_key)
VALUES ('class', '95', 'cheaters.view')
ON CONFLICT (role_type, role_key, permission_key) DO NOTHING;
