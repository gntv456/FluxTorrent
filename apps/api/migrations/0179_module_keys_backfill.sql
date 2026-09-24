-- 0179：site-profile 模块缺省口径对齐（二审 R1 收尾）。
--
-- 0178 已把 modules 注册表与 site_settings.module_* 对齐 general 中立矩阵；
-- 本迁移做两件收尾：
-- 1) settings_meta 里模块开关卡片若登记过「默认值」提示文案，同步为中立口径；
-- 2) 自定义站型包（custom_*）若是在旧缺省时代另存的（modules 快照可能只含
--    当时落库的部分键），不动——apply 仍按快照显式值工作，缺键回落新 default_on。
-- 实质上是空操作迁移：占位登记本轮「缺省翻转」的迁移语义，供后续审计定位。

-- 兜底：确保 29 键全部显式在库（0107 落过 24 键；0178 补齐对齐后的全集）。
INSERT INTO site_settings (name, value, descr, grp)
SELECT 'module_' || key, CASE WHEN is_on THEN 'yes' ELSE 'no' END,
       name_zh || '（模块开关）', 'module'
FROM modules
ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, updated_at = now();
