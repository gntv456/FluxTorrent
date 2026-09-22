-- 0154 论坛管理改版（见 _doc/论坛管理后台改版策划案.md）：
-- 1) 分区名唯一（lower + btrim）——防「分区/版块同名」的认知歧义
-- 2) forums.sort —— 版块在分区内的排序（原表无此列，顺序只能按 id）
-- 3) 两处 sort 回填 —— 现状 forum_categories.sort 全为 0，顺序不稳
-- 4) idx_forums_cat_sort —— 支撑「按分区+分区内顺序」查询
-- 5) 后台导航条目 forums 指向独立整页 /admin/forums
-- 全部幂等（IF NOT EXISTS / 幂等 UPDATE），可重复执行。

-- ---------- 1) 分区名唯一 ----------
-- 注意：若库中已有重名分区，本语句会失败。上迁移前先跑：
--   SELECT lower(btrim(name)), count(*) FROM forum_categories GROUP BY 1 HAVING count(*) > 1;
CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_categories_name
  ON forum_categories (lower(btrim(name)));

-- ---------- 2) 版块分区内排序 ----------
ALTER TABLE forums ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 0;

-- ---------- 3) 回填 ----------
-- 3a) 分区内版块按现有 id 顺序铺开 sort（幂等：只在全 0 时执行）
UPDATE forums f SET sort = r.rn
FROM (
  SELECT id, (row_number() OVER (PARTITION BY category_id ORDER BY id) - 1) AS rn
  FROM forums
) r
WHERE f.id = r.id
  AND NOT EXISTS (SELECT 1 FROM forums WHERE sort <> 0);

-- 3b) 分区 sort 去重（仅当所有分区 sort 相同时重排，避免覆盖用户已调好的顺序）
UPDATE forum_categories c SET sort = r.rn
FROM (
  SELECT id, (row_number() OVER (ORDER BY sort, id) - 1) AS rn
  FROM forum_categories
) r
WHERE c.id = r.id
  AND (SELECT count(DISTINCT sort) FROM forum_categories) <= 1;

-- ---------- 4) 支撑索引 ----------
CREATE INDEX IF NOT EXISTS idx_forums_cat_sort ON forums (category_id, sort, id);

-- ---------- 5) 后台导航指向独立整页 ----------
UPDATE staff_panel_entries
SET url = '/admin/forums'
WHERE tab_key = 'forums' AND url = '/admin?tool=forums';
