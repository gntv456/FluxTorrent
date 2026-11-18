-- 0496 契约计量升级：消灭「近似口径罚款」
--
-- 背景（产品体检 2026-11-18 A1）：seed 契约的 achieved 用「结算时刻在种体积 ×
-- 月内活跃天数占比」近似月均做种体积——snatches 只有现值没有历史，挂 10 天
-- 种 3TB 的用户会被按结算日快照计。契约会扣罚，近似口径 + 罚款 + 用户月内
-- 看不到进度 = 投诉源。本迁移建日采样表，hourly 快照、结算改真均值：
--
--   1) seed_contract_samples(user_id, day, bytes)：每日一条在种体积快照
--      （站点日口径，与签到/趋势图同源）。结算 achieved = 月内日均。
--   2) 同步落「progress」读口：GET /contracts/tiers 响应带当月实时进度，
--      用户月内随时可见「当前日均 x / 目标 y」。
--
-- 站长不升级前端也能受益：结算口径自动变准（worker 同批改造）。

-- 1) 采样表：PK(user_id, day) 天然幂等，重复采样 ON CONFLICT 覆盖
CREATE TABLE IF NOT EXISTS seed_contract_samples (
    user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day     DATE   NOT NULL,
    bytes   BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, day)
);

-- 只在「有 open seed 契约的用户」子集上采样：部分索引让 hourly job 的
-- UPDATE 走索引、也让无契约用户零成本。
CREATE INDEX IF NOT EXISTS seed_contract_samples_user_idx
    ON seed_contract_samples (user_id, day);

-- 2) settings_meta 登记读侧新键（对齐 0423 的写法）：无新键，仅注释说明
--    进度读口复用 GET /contracts/tiers 的 eligible/enabled，不新增设定键。
