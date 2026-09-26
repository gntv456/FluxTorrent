-- 0215：站名/副标题死键退役（0214 站名收敛的收尾）
--
-- 现状：site_title（0025 引入，旧值 baozi 已被 0120 中性化）、site_subtitle
-- 两个键全站零读取（前端/后端 grep 无消费点；settings_meta 仍渲染成可编辑
-- 字段——站长改了没有任何效果，属于「设置页假开关」）。
-- 处理：从 settings_meta 与 site_settings 退役（保留 SITENAME——RSS 频道名
-- 在读，且 0214 已做 site_name→SITENAME 单向镜像）。effects_for 里的
-- "site_title" 分支在键消失后自然不再命中，无需改代码。
DELETE FROM settings_meta WHERE name IN ('site_title', 'site_subtitle');
DELETE FROM site_settings WHERE name IN ('site_title', 'site_subtitle');
