-- 0344_staff_panel_module_key_null.sql
-- P-25 存量归一：staff_panel_entries.module_key 的空串改成 NULL。
--
-- 导航判据 modules.rs::module_on_sql 旧版只认 `module_key IS NULL` 为
-- 「不挂模块」，空串于是被当成一个查不到开关的模块键，按「未配置按关
-- 处理」整行不返回。后台「面板条目」写路径一直把 '' 归一成 NULL，规范
-- 值是 NULL；只有迁移里手写这一列（填 '' 更顺手）的人会被静默吞掉——
-- 0339 的 trumps 与 0342 的 classes 两条导航项自落地以来从未出现过。
--
-- 同批判据也改为容忍 ''（读写两侧同语义），这里把存量两行归到规范值。
-- 0343 的文案段已应用过、不可再改（改字节就撞 sqlx checksum，站点起
-- 不来），故另记一笔。

BEGIN;

UPDATE staff_panel_entries
   SET module_key = NULL
 WHERE module_key = '';

COMMIT;
