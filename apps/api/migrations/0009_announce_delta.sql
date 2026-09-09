-- P0 修复：announce 上报的是客户端累计量，计费必须转增量。
-- snatches 增加 last_up/last_down 上次上报值；计费 delta = max(0, cur - last)。
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS last_up BIGINT NOT NULL DEFAULT 0;
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS last_down BIGINT NOT NULL DEFAULT 0;
