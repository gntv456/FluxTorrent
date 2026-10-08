-- 0304（贴边汇报检测，2026-10-07 网盘挂 NAS 挂机型假做种专杀）
--
-- 手法：网盘/存储平时撤掉，只在汇报前临时挂上——客户端常开（回连端口活、
-- conn=1 探测测不出数据不在盘），announce 间隔长期贴着做种时长容忍上限
-- （seed_cap = 2×interval，缺省 3600s）之下：挂 ~3400-3590s 汇报一次，
-- 每轮照拿满间隔的 seeded_seconds。正常客户端按 interval（1800s）汇报，
-- tracker 抖动/休眠只是偶发贴边，不会持续。
-- 检测：snatches 上加两个计数列，process_event 在同一事务内累计——
--   · total_announces   有效做种汇报次数（间隔 ≤ cap 且本次在种）
--   · near_cap_announces 贴边次数（间隔 > 0.85 × seed_cap）
-- collusion_check 的 near_cap_check 画像：近窗口内贴边占比 ≥ 80% 且
-- 样本 ≥ 20 次 → cheat_events（只记录进处置待办，不自动处罚——
-- 弱网/移动端用户可能长期高间隔，留人工裁量）。
-- 全部幂等（IF NOT EXISTS），重跑安全。

ALTER TABLE snatches ADD COLUMN IF NOT EXISTS total_announces integer NOT NULL DEFAULT 0;
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS near_cap_announces integer NOT NULL DEFAULT 0;
COMMENT ON COLUMN snatches.total_announces IS
'有效做种 announce 次数（间隔在容忍窗内计入时长的汇报）：贴边汇报节奏检测的分母。';
COMMENT ON COLUMN snatches.near_cap_announces IS
'贴边汇报次数（间隔 > 0.85×seed_cap 但仍在窗内计入时长）：网盘临时挂载型假做种的节奏指纹。占比持续 ≥80% 即命中画像（见 worker collusion_check）。';
