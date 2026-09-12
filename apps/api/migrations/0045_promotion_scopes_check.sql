-- 种子促销（M49）形状约束：torrent 级促销必须挂 torrent_id；
-- 全站/官种/非官种/分类促销必须不挂 torrent_id（分类促销用 category_id 圈定范围）。
ALTER TABLE promotions ADD CONSTRAINT promotions_scope_shape_check
  CHECK ((scope IN ('global', 'official', 'non_official', 'category')) = (torrent_id IS NULL));
