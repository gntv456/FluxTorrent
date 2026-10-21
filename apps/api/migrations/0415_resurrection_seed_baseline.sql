-- 0415: 单人复活任务记录领取时刻的做种基线（P-483）
--
-- 病形：worker 的 resurrection_settle 用 snatches.seeded_seconds 的**累计值**
-- 判达标。一个已经在热门种上挂了 240h 的会员，领复活任务后下一跳就被选中
-- 发奖（5000 魔力 + 免费券 + 7 天免费），没有补种一天。组队契约那边一直
-- 用 social_team_member.seed_seconds_begin 的增量口径，单人是唯一的例外。
--
-- 修法：领取时把该用户在该种上的既有做种秒数快照进本列；验收要求
-- seeded_seconds >= seed_base_seconds + required_hours*3600，即只认领取后
-- 新增的做种时长。历史行（本列默认 0）语义不变。
ALTER TABLE resurrections
    ADD COLUMN IF NOT EXISTS seed_base_seconds BIGINT NOT NULL DEFAULT 0;
