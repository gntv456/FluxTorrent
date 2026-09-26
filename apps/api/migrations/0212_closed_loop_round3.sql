-- 0212：闭环 Round3 修复批（2026-09-26 三审遗留项）
--
-- C11：定向众筹后台管理——此前站长无列表/取消入口，网关掉单或需人工干预
--   时只能改库。本迁移只补导航条目种子（端点见 economy_http/funding.rs 的
--   /admin/fundings 与 /admin/fundings/cancel，无新表/新列）。
-- C13：批量发放补邮件通知——复用 mailer::notify，无新列（请求体加 email 布尔）。
-- B10：适配器卸载端点——复用 POST /admin/adapters/delete，无新列。
--
-- 幂等：WHERE NOT EXISTS 守卫；重复执行安全。

-- C11 导航条目：众筹管理（运维域 section='ops'，见 0211 H1 的 section 白名单）
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'admin', '众筹管理', '/admin?tool=fundings',
       '定向众筹列表 / 进度 / 关闭（可退款）',
       14, 'ops', 93, 'fundings', 'staff.panel'
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries WHERE tab_key = 'fundings'
);
