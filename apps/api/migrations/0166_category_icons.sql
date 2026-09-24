-- 0166：分类图标键（TIDE 图标系统配套）。
-- categories.icon_key → 前端 Icon 组件的语义名；站长可在后台改键或留空
-- （留空回落分类名首字色块）。默认按站型分类语义铺一套初始映射。
ALTER TABLE categories ADD COLUMN IF NOT EXISTS icon_key TEXT NOT NULL DEFAULT '';

-- 默认映射：按现有常见分类名铺装（不存在的行零副作用）
UPDATE categories SET icon_key = 'film'      WHERE icon_key = '' AND (name LIKE '%电影%' OR name LIKE '%影院%' OR name LIKE '%BluRay%' OR name LIKE '%Remux%');
UPDATE categories SET icon_key = 'tv'        WHERE icon_key = '' AND (name LIKE '%电视%' OR name LIKE '%剧%' OR name LIKE '%综艺%');
UPDATE categories SET icon_key = 'music'     WHERE icon_key = '' AND (name LIKE '%音乐%' OR name LIKE '%无损%' OR name LIKE '%FLAC%');
UPDATE categories SET icon_key = 'anime'     WHERE icon_key = '' AND name LIKE '%动漫%';
UPDATE categories SET icon_key = 'game'      WHERE icon_key = '' AND name LIKE '%游戏%';
UPDATE categories SET icon_key = 'app'       WHERE icon_key = '' AND (name LIKE '%软件%' OR name LIKE '%工具%');
UPDATE categories SET icon_key = 'book'      WHERE icon_key = '' AND (name LIKE '%电子书%' OR name LIKE '%书%' OR name LIKE '%课本%');
UPDATE categories SET icon_key = 'sport'     WHERE icon_key = '' AND name LIKE '%体育%';
UPDATE categories SET icon_key = 'doc'       WHERE icon_key = '' AND (name LIKE '%纪录%' OR name LIKE '%教育影音%');
UPDATE categories SET icon_key = 'edu'       WHERE icon_key = '' AND (name LIKE '%学%' OR name LIKE '%教育%' OR name LIKE '%课程%');

-- 管理面可编辑（rules_cats 同款模式）：settings_meta 不需要——categories 走
-- /admin/categories CRUD，icon_key 字段由后端 put 端点接收。
