-- 0043 保种时长数据源修复：snatches.seeded_seconds 自 0001 建列以来全仓只读不写，
-- 导致 H&R 追责（hr_snapshots 同步）、等级做种小时、保种认领基线、/me/hr 全部拿到恒 0。
-- 新增 last_seen_at：worker 在 announce 计费 upsert 时按「相邻两次做种 announce 的时间差」
-- 累计做种秒数（含容忍窗上限，见 worker/src/jobs.rs process_event）。
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now();
