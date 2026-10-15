-- 0368_risk_score_m2.sql：反作弊风险评分 M2（对标 PT Studio FR-ACH-010 简化版）
-- 策略：固定公式（不用 JSON DSL 规则集），权重/阈值全部 site_settings 化。
--   score = min(100, round( Σ_家族 w_a × f(hits_a) × decay(days_a) × 100 / NORM ))
--   f(hits) = 1 + log2(min(hits,16))；decay = 2^(−days_since_last / 半衰期)
-- 分段：none(<low) / low / watch / high / critical；watch+ 进观察名单，
-- high/critical 且无进行中案件 → 自动开案（priority 1/2）。
-- 幂等，可重跑。

-- ① 评分表（一用户一行；shadow_breakdown 存各家族贡献分，可解释性）
CREATE TABLE IF NOT EXISTS user_risk_scores (
  user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  score INT NOT NULL DEFAULT 0 CHECK (score BETWEEN 0 AND 100),
  band TEXT NOT NULL DEFAULT 'none',
  shadow_breakdown JSONB,
  ruleset_ver INT NOT NULL DEFAULT 1,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- 观察名单索引：band != 'none' 的部分索引（队列按分倒序）
CREATE INDEX IF NOT EXISTS idx_risk_band
  ON user_risk_scores (band, score DESC) WHERE band <> 'none';
-- 增量重算定位：比 updated_at 新的证据（配合 cheat_events 的 user_id 索引）
CREATE INDEX IF NOT EXISTS idx_risk_updated ON user_risk_scores (updated_at);

-- ② 公式参数（全部可调；键名与 cheat_l1_hits 等同族，缺行回落出厂值）
INSERT INTO site_settings (name, value, descr) VALUES
('risk_norm', '8', '风险分归一化常数 NORM：单家族满配(hits≥16,新鲜,权重1.0)贡献 100/NORM 分'),
('risk_decay_days', '14', '衰减半衰期（天）：score 随证据变旧按 2^(-days/半衰期) 衰减'),
('risk_band_low', '20', 'low 档阈值'),
('risk_band_watch', '40', 'watch 档阈值（观察名单起点）'),
('risk_band_high', '60', 'high 档阈值（达此档且无进行中案件自动开案 priority=1）'),
('risk_band_critical', '80', 'critical 档阈值（自动开案 priority=2）'),
('risk_weight_ghost', '1.0', 'ghost 家族权重（幽灵做种）'),
('risk_weight_speed', '0.8', 'speed 家族权重（速率超限）'),
('risk_weight_reset', '1.0', 'reset 家族权重（计数器回退）'),
('risk_weight_corr', '0.6', 'corr 家族权重（自报上传超佐证上界）'),
('risk_weight_xreport_over', '0.8', 'xreport_over 家族权重（佐证量超物理额度）'),
('risk_weight_nearcap', '0.3', 'nearcap 家族权重（贴边汇报画像）'),
('risk_weight_self_deal', '0.5', 'self_deal 家族权重（cheat_events 的 deal: 前缀）'),
('risk_weight_down_under', '0.4', 'down_under 家族权重'),
('risk_weight_bitthief', '0.7', 'bitthief 家族权重'),
('risk_weight_default', '0.5', '未列名家族的默认权重（torrent:/login_ip_spread/connectable 等）')
ON CONFLICT (name) DO NOTHING;
