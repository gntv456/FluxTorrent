-- 0153 列表排序复合索引（六维强化方案批次二）
--
-- 前提：列表排序语义改为「用户显式排序不掺置顶」（置顶只在默认浏览顺序生效，NP 同口径）。
-- 此前 ORDER BY 首列是含 now() 的置顶计算表达式（STABLE，PG 拒绝索引），
-- 任何排序都全筛选集 Sort；现在 ORDER BY = (col, id)，这批索引才能真正消除 Sort 节点。
-- 游标同步升级为「(排序列值, id)」二元 keyset（0151 注释里预告的 sticky 物化路径由此绕开：
-- 不再需要 sticky_rank 物化列，表达式只在默认排序使用，其游标值由 SELECT 输出携带）。
--
-- ⚠️ sqlx 迁移在事务内执行，不能用 CREATE INDEX CONCURRENTLY（见 0142 注释）。

CREATE INDEX IF NOT EXISTS idx_torrents_sort_seeders ON torrents (seeders DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_torrents_sort_leechers ON torrents (leechers DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_torrents_sort_size ON torrents (size DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_torrents_sort_completed ON torrents (times_completed DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_torrents_sort_name ON torrents (name, id);
