-- 全文搜索（标题外）：pg_trgm 三元组索引（M02 扩展）
-- 覆盖 name / small_descr / descr / files.path 四列（中文按字符子串匹配，无需分词）。
-- technical_info / nfo 不索引不搜索：OR 臂中任一不可索引会拖垮 BitmapOr 计划（低占用取舍）。
CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS idx_torrents_name_trgm ON torrents USING gin (name gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_torrents_small_descr_trgm ON torrents USING gin (small_descr gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_torrents_descr_trgm ON torrents USING gin (descr gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_files_path_trgm ON files USING gin (path gin_trgm_ops);
