-- 0494：self_deal 共现门槛键补播种（5509b610 收口遗漏）。
--
-- 5509b610（2026-11-16）给 deal 检测加了「≥N 颗种同向共现才立案」的
-- CGNAT 去噪门槛，代码读 site_settings.collusion_deal_min_swarms
-- （出厂 3，clamp 1..50，DB 无行时兜底 3），CHANGELOG 也承诺「可调」——
-- 但没写迁移：键从未播种、settings_meta 无登记，管理后台看不见也调不
-- 了，只能手改 DB。对照同族惯例（collusion_tolerance_ratio_pct /
-- collusion_min_gap_gb 走 0311 的 site_settings+settings_meta 双写），
-- 这里补齐两表。

INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('collusion_deal_min_swarms',
   '3',
   'self_deal（对刷）立案的共现种数门槛：同一账号对在 30 分钟窗内'
   '至少 N 颗种子出现同 IP 同向共现才立案（CGNAT 去噪——家庭宽带'
   '共享出口 IP 上偶发的单种共现不立案）。出厂 3，可调 1..50。',
   'anticheat')
ON CONFLICT (name) DO NOTHING;

-- anticheat 区 card_order 已用到 32（0372），接 33。
-- min_class 显式 98：建表缺省是 99，而 0493 刚把全表 99→98 拉平
-- （维护开发员≈站长的设置写权口径）——不显式给值，本键会以 99
-- 落地，重新制造 0493 要修的不一致。
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order,
   visible, min, max, step, min_class)
VALUES
  ('collusion_deal_min_swarms', 'number',
   '对刷共现种数门槛', 'Self-deal co-occurrence threshold',
   '同一账号对在 30 分钟窗内 ≥N 颗种子同 IP 同向共现才立案对刷。'
   '调高更宽松（漏报多），调低更严格（CGNAT 误报多）。',
   '颗', 'anticheat', 33, true, 1, 50, 1, 98)
ON CONFLICT (name) DO NOTHING;
