-- 0528 保种区性能索引（2026-11-28）：好学站新口径灌入 11.7k 在区行后，
-- preserve 列表查询 27s+（无索引支撑的过滤+排序全表扫描），页面骨架屏
-- 超过 Next fetch 超时 → 永不落地。三个部分索引覆盖列表/统计/清洗查询：
--
-- ① 列表主查询：在区过滤 + seeders 排序（preserve_list ORDER BY 主路径）
CREATE INDEX IF NOT EXISTS idx_seed_preserve_in_zone_seeders
  ON seed_preserve (torrent_id) INCLUDE (seed_time_begin, uploaded_begin)
  WHERE exited_at IS NULL;
-- ② 已出区查询（grace/out 视图档 + preserve_exit worker 扫描）
CREATE INDEX IF NOT EXISTS idx_seed_preserve_exited
  ON seed_preserve (torrent_id) WHERE exited_at IS NOT NULL;
-- ③ 结算 worker（preserve_settle 取 claimed_by 非空且在区）
CREATE INDEX IF NOT EXISTS idx_seed_preserve_claimed_active
  ON seed_preserve (claimed_by) WHERE exited_at IS NULL AND claimed_by IS NOT NULL;
