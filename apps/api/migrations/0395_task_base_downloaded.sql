-- P-344：download_delta 从来没有认领时基线。task_claims 里只有
-- base_uploaded / base_seed_seconds / base_uploads 三列，结算时
-- `met &= downloaded >= v` 直接拿 users.downloaded 现值比——于是
-- 「非 tier 任务配一个总下载量」＝认领即达标（存量下载早就超过任
-- 何合理门槛）白拿奖励。tier（等级系列）任务按设计就是累计口径，
-- 行为不变；本列只服务非 tier 任务的增量判定。
-- 存量认领行回落 0＝与迁移前逐位一致（现网 download_delta 只出现在
-- tier 任务上，不受影响；新认领由写口捕获真实基线）。
ALTER TABLE task_claims
    ADD COLUMN IF NOT EXISTS base_downloaded BIGINT NOT NULL DEFAULT 0;

COMMENT ON COLUMN task_claims.base_downloaded IS
    '认领时的 users.downloaded 快照；非 tier 任务的 download_delta 增量基线';
