-- 0342_staff_panel_classes.sql
-- 管理台导航：等级规则（P-19）。
--
-- 背景：`admin_p3_http/classes_admin.rs` 早就有一读一写两个端点
-- （GET /admin/classes 带当前人数、PUT /admin/classes/{id} 锁 staff 档），
-- 但**管理台没有入口**——站长只能直连 API 改阈值。后果不止不便：
-- `class_rules.promo_sparks` 因此常年为 0，worker `class_auto_adjust` 的
-- 晋升奖励发放与「等级晋升祝贺」信（0072 整条实现）等于永久空转。
-- 本批闸门 scripts/pt_audit_ai_classes.py 逐出口钉这条链：设阈值 →
-- 达标成员 → 真跑一轮 → 档位 / 余额 / 流水幂等键 / 祝贺信四个出口，
-- 并断言再跑一轮不重复入账。
--
-- 同批修 P-23：GET /admin/classes 原先直接读 class_rules，把只在
-- user_classes 里存在的 8 档（贵宾 20、职务档 91+）连同「staff 档
-- 只读展示」这句注释一起变没了；档位名也取的是 class_rules 别名
-- （LV1/LV2…），而成员档案与公开等级页显示 user_classes.name
-- （User/Power User…）——站长改的与成员看到的不是同一列。现全集
-- 按 user_classes 出，无规则行的档位前端显示「—」且 PUT 拒绝。
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
       '成长档升降级阈值与晋升奖励（贵宾/职务档只读）',
       (SELECT COALESCE(max(sort), 0) + 1 FROM staff_panel_entries
          WHERE section = 'users'),
       'users', 93, 'classes', '', 'sitepacks.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'classes'
 );

COMMIT;
