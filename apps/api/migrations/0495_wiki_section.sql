-- 0495：wiki 分组目录（section 列）。
--
-- wiki 目录页从平铺清单升级为分组卡片（start 快速上手 / site 玩转站点 /
-- rules 制度与规则 / api 软件与 API），页面归组由 section 列承载；
-- 缺省 ''（未分组），旧数据零迁移——目录页对空组单独归档展示，
-- 后台 CRUD 的 WikiBody 增加可选 section 字段（缺省 ''）。

ALTER TABLE wiki_pages
  ADD COLUMN IF NOT EXISTS section text NOT NULL DEFAULT '';
