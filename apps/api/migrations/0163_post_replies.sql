-- 0163 楼中楼（两级嵌套，知乎式口径）：
--   顶层楼 root_id = NULL；楼中楼 root_id = 所属顶层楼 id，parent_id = 直接回复的目标楼。
-- posts 是按 created_at 的分区表，ALTER 会传播到全部分区；
-- 分区表外键受限，归属校验放应用层（reply 时校验目标楼属于同主题）。
ALTER TABLE posts ADD COLUMN IF NOT EXISTS root_id BIGINT;
ALTER TABLE posts ADD COLUMN IF NOT EXISTS parent_id BIGINT;
CREATE INDEX IF NOT EXISTS idx_posts_root_id ON posts (root_id) WHERE root_id IS NOT NULL;
