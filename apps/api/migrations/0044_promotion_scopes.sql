-- 种子促销（M49）：促销范围扩展 —— 全站 / 官种 / 非官种 / 分类
-- 旧 promotion_scope 只有 torrent|global；新增三个"条件性全站"范围。
-- 官种判定沿用 torrents.official_tag（与 tags 表 官种=3 同口径）。
-- 注意：新增枚举值不能在添加它的同一事务中被引用，形状约束重建放到 0045。
ALTER TYPE promotion_scope ADD VALUE IF NOT EXISTS 'official';
ALTER TYPE promotion_scope ADD VALUE IF NOT EXISTS 'non_official';
ALTER TYPE promotion_scope ADD VALUE IF NOT EXISTS 'category';

ALTER TABLE promotions ADD COLUMN IF NOT EXISTS category_id INT REFERENCES categories(id);

ALTER TABLE promotions DROP CONSTRAINT IF EXISTS promotions_torrent_id_check;
ALTER TABLE promotions DROP CONSTRAINT IF EXISTS promotions_check;

CREATE INDEX IF NOT EXISTS idx_promotions_active_scoped
  ON promotions (scope, category_id, ends_at);
