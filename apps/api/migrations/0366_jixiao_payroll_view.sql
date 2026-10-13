-- 0366：jixiao.payroll.view（批六十六 P-245，2026-10-13）
--
-- 现网红证（闸门 scripts/pt_audit_cf_exam_audit_tiers.py 的 CF2/CF4b）：
--   GET /admin/jixiao/overview 与 GET /admin/jixiao/payroll 只判 `staff()`，
--   返回的是**每个成员的姓名 + 金额 + 奖金 + 结算时间**。一个 class 12 的
--   成员、只在用户级覆盖了 staff.panel（**低于一切职员档**）实测：
--     overview=200 内容=username=cft23347 amount=0
--     payroll=200
--   `staff.panel` 挂在 class/90，而 class 键是「id >= role_key」的阈值语义
--   ⇒ 91 发布员、92 版主都能把全站的钱袋子明细整页读走。
--
-- 档位（读别人的工资 = 与 0285 user.passkey.reveal、批六十三 sparklog.view、
--   批六十四 subtitles.grant、批六十五 jixiao.assign 同档 95）：
--   jixiao.payroll.view class/95 —— 全站发薪明细
--   `/admin/jixiao/overview` 不再新设键：它管的是「考核进度」，而这套页面的
--   日常使用者正是**能分配考核的人** ⇒ 接已有的 `exam.manage`（0365 前就在，
--   授 class/93）。少一个键少一处「后台能配、代码不接」的漂移面。
-- 幂等；implemented=true（代码侧已接线，CF7 的三件齐判据盯住只加键不接线）。

INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('jixiao.payroll.view', '查看全站发薪明细', 'jixiao',
 '读取每名成员本期领了多少考核薪酬、由谁发放、何时结算', 77, true)
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_type, role_key, permission_key) VALUES
('class', '95', 'jixiao.payroll.view')
ON CONFLICT (role_type, role_key, permission_key) DO NOTHING;
