-- 0470 搜索性能索引（0473/0475 配套）：教育四级导航回填后，搜索/筛选
-- 查询在大表上裸 ILIKE 全表扫，实测单次 77s（files 380 万行、descr 全站
-- 简介文本 ~700MB），多次并发搜索即可打满 4 核。
-- pg_trgm GIN 让 ILIKE '%kw%' 走索引。
--
-- 注意（0151 同款惯例）：sqlx 迁移在事务内执行，无法 CREATE INDEX
-- CONCURRENTLY。大表存量站点先由运维 CONCURRENTLY 预建同名索引后再
-- 启动（本文件的 IF NOT EXISTS 会跳过）；新部署小库直接建，秒级完成。
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- ① 教育筛选/聚合按 torrent_id 的谓词（0473 教育四级导航路径；
--    此前 torrent_sections 无 torrent_id 索引，列表页每行聚合全表扫）
CREATE INDEX IF NOT EXISTS idx_torrent_sections_tid
  ON torrent_sections (torrent_id);

-- ② 签到页按日 count（attendance 迁移后 65 万行）
CREATE INDEX IF NOT EXISTS idx_attendance_date ON attendance (date);

-- ③ 搜索三件套：文件名（380 万行）/ 简介 / 副标题 / 标题
CREATE INDEX IF NOT EXISTS idx_files_path_trgm
  ON files USING gin (path gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_torrents_descr_trgm
  ON torrents USING gin (descr gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_torrents_sd_trgm
  ON torrents USING gin (small_descr gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_torrents_name_trgm
  ON torrents USING gin (name gin_trgm_ops);
