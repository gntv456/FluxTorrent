-- 0042 第六轮：保种认领对齐 NP claims 口径（好学站 user/claims 后台入口）
-- NP claims 表核心列：uid / torrent_id / seed_time_begin / uploaded_begin / last_settle_at
-- FT seed_preserve 已有 claimed_by/claimed_at/exited_at；补基线列，认领后可核算「本月做种时长/上传增量」。
-- 做种时长复用 snatches.seeded_seconds（0001 已有，秒），不另加 seed_time 列。

ALTER TABLE seed_preserve
    ADD COLUMN IF NOT EXISTS seed_time_begin BIGINT NOT NULL DEFAULT 0,      -- 认领时做种秒数基线（snatches.seeded_seconds）
    ADD COLUMN IF NOT EXISTS uploaded_begin BIGINT NOT NULL DEFAULT 0,       -- 认领时上传量基线（snatches.uploaded）
    ADD COLUMN IF NOT EXISTS last_settle_at TIMESTAMPTZ;                      -- 最近结算时间（NP last_settle_at）
