-- 施肥（样图⑤「施肥 2/3」的落地）：每茬一次，催熟 30 分钟（浇水是 10 分钟）。
-- 费用走 eco 键 farm_fertilize_spark（缺省 5，0 = 免费），与浇水同口径。
ALTER TABLE farm_plots
    ADD COLUMN IF NOT EXISTS fertilized BOOLEAN NOT NULL DEFAULT FALSE;

-- 施肥周常（与 0247 的 q_farm 浇水任务同族；ref_type='farm_fertilize'
-- 由 fertilize 端点的 spend_spark 写入，arcade_meta 的周常聚合自动计入）
INSERT INTO arcade_quests (code, game_ref, target, reward_spark, sort) VALUES
    ('q_farm_fert', 'farm_fertilize', 2, 120, 35)
ON CONFLICT (code) DO NOTHING;
