-- 0158 合集 Collection / 系列 Series（六维强化方案 阶段三「聚合层」）
-- （原写 0157，与并行会话 0157_comment_replies 撞号，让位重编号；内容未动）
--
-- 与既有 torrent_groups（0069，同资源多版本）互补的三层聚合：
--   版本组 group     —— 一部作品内的年份/版本/清晰度（已有）
--   合集 collection  —— 策展集合：专题/榜单/课程包，一 种可入多合集（M-TX 卡片墙口径）
--   系列 series      —— 作品级序列：季/部/卷，一个系列含多个版本组（UNIT3D 口径）
-- 建模：collections 一张表（kind 区分 collection/series），torrent_collections 多对多。
-- series→group 的从属用 torrent_groups.series_id 表达（组挂系列，不是种直接挂）。

CREATE TABLE IF NOT EXISTS collections (
  id BIGSERIAL PRIMARY KEY,
  kind TEXT NOT NULL DEFAULT 'collection'
    CHECK (kind IN ('collection', 'series')),
  name TEXT NOT NULL,
  descr TEXT,
  cover TEXT,
  category_id INT,
  sort_order INT NOT NULL DEFAULT 0,
  created_by BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (kind, name)
);

CREATE TABLE IF NOT EXISTS torrent_collections (
  collection_id BIGINT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  added_by BIGINT REFERENCES users(id),
  added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (collection_id, torrent_id)
);

CREATE INDEX IF NOT EXISTS idx_torrent_collections_t
  ON torrent_collections (torrent_id);

CREATE INDEX IF NOT EXISTS idx_collections_kind
  ON collections (kind, sort_order);

ALTER TABLE torrent_groups
  ADD COLUMN IF NOT EXISTS series_id BIGINT REFERENCES collections(id)
    ON DELETE SET NULL;
