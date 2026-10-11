-- 0342_staff_panel_classes.sql
-- 管理台导航：等级规则（P-19）。
--
-- 背景：`admin_p3_http/classes_admin.rs` 早就有一读一写两个端点
-- （GET /admin/classes 带当前人数、PUT /admin/classes/{id} 锁 staff 档），
-- 但**管理台没有入口**——站长只能直连 API 改阈值。后果不止不便：
-- `class_rules.promo_sparks` 因此常年为 0，worker `class_auto_adjust` 的
-- 晋升奖励发放与「等级晋升祝贺」信（0072 整条实现）等于永久空转。
-- 2026-10-11 实测：把档位 3 的 promo_sparks 设成 70 后，达标成员一次
-- 跑批即 1→3 晋升、入账 70 火花并收到祝贺信，且再跑一轮不重复入账——
-- 链路本身是好的，缺的只是界面。
--
-- 段归属：放 `users`（与职务管理/邀请管理/签到记录同族，都是成员养成
-- 侧），不放 `system`（那段是日志/数据库/权限等基础设施）；sort 取段内
-- 最大值 +1，不与既有排序撞位。
--
-- ⚠️ `tab_key` 必须与 `renderSimpleTool` 的 case、i18n 命名空间一致，
-- 该一致性现由 scripts/admin_nav_align_guard.py 守（本批补的门禁）。
-- 幂等：WHERE NOT EXISTS。

BEGIN;

INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     module_key, perm_key)
SELECT 'admin', '等级规则', '/admin?tool=classes',
       '成员档升降级阈值与晋升奖励（staff 档只读）',
       (SELECT COALESCE(max(sort), 0) + 1 FROM staff_panel_entries
          WHERE section = 'users'),
       'users', 93, 'classes', '', 'sitepacks.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'classes'
 );

COMMIT;
