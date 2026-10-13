-- 0351 agent_rules 版本区间 + 优先级：
-- 规则可限定客户端版本范围（点分数字，如 4.6 / 5.0.1）并指定优先级。
-- 先例：0071 的 peer_id_pattern TEXT NOT NULL DEFAULT ''（同表同口径）。
-- version_min/version_max 空 = 不限版本；priority 0..=1000，大者先判
--（tracker 侧按 priority DESC 稳定排序，相同 priority 保持加载序）。
ALTER TABLE agent_rules ADD COLUMN IF NOT EXISTS version_min TEXT NOT NULL DEFAULT '';
ALTER TABLE agent_rules ADD COLUMN IF NOT EXISTS version_max TEXT NOT NULL DEFAULT '';
ALTER TABLE agent_rules ADD COLUMN IF NOT EXISTS priority INT NOT NULL DEFAULT 0;
