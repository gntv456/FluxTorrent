-- 0274：帮助中心分组（站内 wiki 第一阶段）
--
-- 背景：docs/user/ 七篇用户指南要进站成为「帮助中心」。custom_pages（0187）
-- 只能存单页、不分组，导航只能靠 menu_items 手工挂，套一套文档时很别扭。
--
-- 做法（wiki-plan.md 方案 A 做法 1）：复用 custom_pages，加两个**可空**列做分组排序。
-- 选可空而非 NOT NULL DEFAULT，是为了让既有 0187 的行（以及 admin 端不传这两个
-- 字段的旧请求）行为完全不变——普通自定义页面 doc_group 为 NULL，不进 /help 目录。
--
-- doc_sort 与既有 sort 的分工：
--   sort      —— 0187 原生，全站自定义页共用（菜单排序沿用它）
--   doc_sort  —— 本次新增，仅 help 目录内部排序；NULL 时回落 sort
--
-- 反向索引：/help 目录页要按 group 取全量可见页，走 idx_custom_pages_sort
-- 已够（该站页面量在三位数），不额外建索引。

BEGIN;

ALTER TABLE custom_pages
    ADD COLUMN IF NOT EXISTS doc_group TEXT,
    ADD COLUMN IF NOT EXISTS doc_sort  INT;

COMMENT ON COLUMN custom_pages.doc_group IS
    '帮助中心分组（如 guide / account / rules）；NULL = 普通自定义页，不进 /help 目录';
COMMENT ON COLUMN custom_pages.doc_sort IS
    '帮助中心目录内排序；NULL 时回落 sort';

-- 目录页按分组聚合的稳定顺序：group 升序、组内 doc_sort 升序。
-- 覆盖 (doc_group, doc_sort) —— 目录页每次整组拉取，索引顺序扫描优于排序。
CREATE INDEX IF NOT EXISTS idx_custom_pages_doc_group
    ON custom_pages (doc_group, doc_sort)
    WHERE doc_group IS NOT NULL;

COMMIT;
