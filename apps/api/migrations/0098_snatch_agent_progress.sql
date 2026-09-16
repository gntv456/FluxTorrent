-- 0098 下载列表增强：BT 客户端与实时进度（NP viewsnatches 口径）
-- agent    = announce 上报的客户端 UA（worker 从事件流写入）
-- progress = 下载进度，万分比 0-10000（worker 由 (size-left)/size 折算；做种恒 10000）
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS agent text NOT NULL DEFAULT '';
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS progress integer NOT NULL DEFAULT 0;
