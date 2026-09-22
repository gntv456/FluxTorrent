-- 0151 种子页缺失索引（六维强化方案 P0-1）
--
-- 背景：详情页 /files 与 /thanks 两个端点按 torrent_id 查询，此前无任何对应索引
-- （files 只有 idx_files_path_trgm 的 path trgm 索引，thanks 完全没有），
-- 每次打开详情页都会对这两张表做顺序扫描；tags 表 PK 是 (torrent_id, tag_id)，
-- 按 tag_id 单列过滤走不到前导列。
--
-- ⚠️ 排序索引（seeders/size/times_completed/created_at）**不在本条迁移里**：
-- 列表 ORDER BY 首列 sticker 表达式含 now()（pos_state_until 到期判定），
-- 属 STABLE 而非 IMMUTABLE，PostgreSQL 不允许建索引；必须先把它物化成 sticky_rank 列，
-- 才能让排序真正走索引。该改造与 keyset 游标对齐一起放在下一批次，避免这里加一批
-- 「看起来有用、实际仍会 Sort」的索引白付写放大。
--
-- ⚠️ sqlx 迁移在事务内执行，不能用 CREATE INDEX CONCURRENTLY（见 0142 注释）：
-- 大表部署请安排在低峰窗口。

-- 详情页：文件列表
CREATE INDEX IF NOT EXISTS idx_files_torrent ON files (torrent_id);

-- 详情页：感谢列表与感谢计数
CREATE INDEX IF NOT EXISTS idx_thanks_torrent ON thanks (torrent_id);

-- 列表筛选：按标签过滤（PK 前导列是 torrent_id，按 tag_id 过滤用不上）
CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags (tag_id);

-- 列表筛选：发布日区间 + 无置顶时的默认次序（与既有的 approval/filter 复合索引互补）
CREATE INDEX IF NOT EXISTS idx_torrents_created_id ON torrents (created_at DESC, id DESC);
