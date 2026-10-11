-- 0343_classes_nav_info.sql
-- 导航项文案纠偏（批十七 P-23 的余波）。
--
-- 0342 的说明写「成员档升降级阈值与晋升奖励（staff 档只读）」，但当时
-- GET /admin/classes 读的是 class_rules：贵宾（20）与职务档（91+）一行
-- 都不返回——「staff 档只读」是一句没有对应数据的承诺。后端本批已改为
-- 按 user_classes 出全集（无规则档只读、门槛显示「—」而不是 0），
-- 这里的文案跟着改成实际范围：成长档可编辑，贵宾/职务档只读。
--
-- 为什么不直接改 0342：它已经跑过（本地库 _sqlx_migrations 有 342 行），
-- 改文件字节就会撞 sqlx 的 checksum，站点下次启动直接起不来。迁移是
-- 账本，写错就再记一笔——与 0341（捐赠套餐文案纠偏）同一处理方式。

BEGIN;

UPDATE staff_panel_entries
   SET info = '成长档升降级阈值与晋升奖励（贵宾/职务档只读）'
 WHERE tab_key = 'classes'
   AND info <> '成长档升降级阈值与晋升奖励（贵宾/职务档只读）';

-- P-25：module_key 的空串是「静默隐形」的坑。判据 `module_on_sql` 旧版
-- 只认 NULL，于是迁移里手写 module_key='' 的导航项（0339 trumps、
-- 0342 classes）在管理台导航里根本不出现——接口 200、页面上没有、
-- 一致性守卫全绿。后台「面板条目」写路径一直把 '' 归一成 NULL，
-- 这里把存量两行也归一（判据同批一并容忍 ''，双保险）。
UPDATE staff_panel_entries
   SET module_key = NULL
 WHERE module_key = '';

COMMIT;
