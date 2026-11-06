-- snatches.times_completed 的部分索引。
--
-- 背景（2026-11-06）：torrents.times_completed 的点刷（全量对账
-- reconcile_snapshots 与 announce 批末点刷）口径是
--   count(*) FROM snatches WHERE torrent_id = ? AND completed_at IS NOT NULL
-- 0464 只补了 seeding / leeching 两个部分索引，completed_at 这一支
-- 仍需全表扫 snatches（迁站后 70 万行 / 147 MB），对账每 6 小时扫一次、
-- announce 每批扫一次，都是纯浪费的 IO 与 buffer 污染。
--
-- 口径不变，只补索引：部分索引 (torrent_id) WHERE completed_at IS NOT NULL
-- 让该聚合走 index-only scan，扫描量与「命中的完成行数」同量级。
-- 幂等：CREATE INDEX IF NOT EXISTS。
CREATE INDEX IF NOT EXISTS idx_snatches_torrent_completed
    ON public.snatches (torrent_id) WHERE completed_at IS NOT NULL;
