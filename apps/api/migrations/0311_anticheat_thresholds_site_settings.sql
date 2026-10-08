-- 0311（硬编码专项审计修复，2026-10-08）
-- 依据：全仓「不该硬编码的硬编码」审计 P1 批。反作弊阈值此前散落在
-- worker SQL/if 分支/常量里，源码级公开——开源后所有站同参可预测。
-- 对策与 0309 同一条纪律：算法公开、密钥私有，参数进 site_settings。
--
-- 本迁移落五组：
-- ① cheat_enforce 累进告警线：L1（staff信箱）hits>=3、L2（标记作弊）hits>=5
-- ② 流量差额审计：相对比率 5x（绝对阈值 cheat_gap_threshold_gb 已存在）
-- ③ 谎报下载检测：容差比率 1.2x 与最小命中差 1GiB（collusion HAVING）
-- ④ 物理上界三件：xreport 佐证单笔 10GiB / 近零重置接受窗 16MiB /
--    快照对账容差 10GiB
-- ⑤ H&R 考察天数兜底：hr_days（种子级 hr_policy->days 未设时的站点缺省，
--    对齐 hr_hours 的三级 COALESCE 模式）
--
-- 全部幂等（ON CONFLICT DO NOTHING），重跑安全。

-- ============ ① 累进告警线 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES
  ('cheat_l1_hits', '3',
   '反作弊累进告警的 L1 线（作弊事件未处置计数达到即向管理组信箱发提醒）。'
   '出厂默认 3 是公开值，建议按本站社区规模调整。',
   'anticheat'),
  ('cheat_l2_hits', '5',
   '反作弊累进告警的 L2 线（达到即直接标记作弊并进入处置流程）。'
   '必须大于 L1；出厂默认 5 是公开值。',
   'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('cheat_l1_hits', 'number', '作弊告警 L1 线', 'Cheat alert L1 hits',
   '作弊事件（未处置）累计到该次数即向管理组发提醒信。开源默认值公开可查，'
   '上线后应调整。',
   '次', 'anticheat', 21, true, 1, 100, 1),
  ('cheat_l2_hits', 'number', '作弊告警 L2 线', 'Cheat alert L2 hits',
   '作弊事件累计到该次数即直接标记作弊并进入处置。应大于 L1 线。',
   '次', 'anticheat', 22, true, 1, 100, 1)
ON CONFLICT (name) DO NOTHING;

-- ============ ② 流量差额审计相对比率 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('cheat_gap_ratio', '5',
        '流量差额审计的相对比率线：窗口内上传总量超过下载总量该倍数才立案'
        '（与绝对阈值 cheat_gap_threshold_gb 取 AND）。出厂默认 5 是公开值。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('cheat_gap_ratio', 'number', '流量差额比率线', 'Traffic gap ratio',
   '7 天窗口内某种子 SUM(up) > 该倍数 × SUM(down) 且超过绝对阈值即立案审计。'
   '比率与绝对阈值同时满足才触发，防小流量误报。',
   '倍', 'anticheat', 23, true, 2, 100, 1)
ON CONFLICT (name) DO NOTHING;

-- ============ ③ 谎报下载检测 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES
  ('collusion_tolerance_ratio_pct', '120',
   '谎报下载检测的容差（百分比）：串通上传总量超过该生下载总量的百分比才立案。'
   '120 = 1.2 倍容差。出厂默认是公开值。',
   'anticheat'),
  ('collusion_min_gap_gb', '1',
   '谎报下载检测的最小命中差（GiB）：串通总量减去下载总量的绝对差须超过该值，'
   '防小量噪音。出厂默认是公开值。',
   'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('collusion_tolerance_ratio_pct', 'number', '串通容差比率', 'Collusion tolerance (%)',
   '串通上传总量 > 下载总量 ×（该值/100）且差值超最小命中差才立案。'
   '调高更宽松（漏报多），调低更严格（误报多）。',
   '%', 'anticheat', 24, true, 100, 500, 5),
  ('collusion_min_gap_gb', 'number', '串通最小命中差', 'Collusion min gap (GiB)',
   '串通总量与下载总量的绝对差低于该值不立案（GiB）。',
   'GiB', 'anticheat', 25, true, 0, 1024, 1)
ON CONFLICT (name) DO NOTHING;

-- ============ ④ 物理上界三件 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES
  ('anticheat_absurd_vouch_gb', '10',
   'xreport 佐证单笔荒谬线（GiB）：单笔佐证量超过该值直接标记荒谬留痕'
   '（tracker 侧已有种子大小物理上界先行丢弃）。出厂默认是公开值。',
   'anticheat'),
  ('anticheat_reset_accept_mb', '16',
   '近零重置接受窗（MiB）：客户端报「重启归零」时，上次读数低于该值才认可'
   '为真重启。六轮审计从 1GiB 收紧到 16MiB（1GiB 实测构成铸币链）。'
   '出厂默认是公开值。',
   'anticheat'),
  ('anticheat_reconcile_tolerance_gb', '10',
   '快照对账容差（GiB）：流水重放与快照余额差超过该值报警。'
   '出厂默认是公开值。',
   'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('anticheat_absurd_vouch_gb', 'number', '佐证荒谬线', 'Absurd vouch (GiB)',
   '单笔 xreport 佐证量上界（GiB），超过即留痕。真正的额度控制在上界账本，'
   '本阈值负责留痕口径。',
   'GiB', 'anticheat', 26, true, 1, 1024, 1),
  ('anticheat_reset_accept_mb', 'number', '重置接受窗', 'Reset accept (MiB)',
   '「客户端重启归零」的可接受上次读数上界（MiB）。调大将重新打开'
   '搬基线铸币口，谨慎。',
   'MiB', 'anticheat', 27, true, 1, 1024, 1),
  ('anticheat_reconcile_tolerance_gb', 'number', '对账容差', 'Reconcile tolerance (GiB)',
   '流水重放与快照余额的容许差额（GiB），超过即告警。',
   'GiB', 'anticheat', 28, true, 1, 1024, 1)
ON CONFLICT (name) DO NOTHING;

-- ============ ⑤ H&R 考察天数站点缺省 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('hr_days', '14',
        'H&R 考察期天数的站点缺省（天）：种子级 hr_policy->days 未设置时使用。'
        '与 hr_hours 同为三级 COALESCE（种子级 → 站点级 → 出厂值）。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('hr_days', 'number', 'H&R 考察天数', 'H&R grace days',
   '完成下载后须在该天数内保持做种（种子级 hr_policy 可按种覆盖）。'
   '过期未满足进入 H&R 违规流程。',
   '天', 'anticheat', 29, true, 1, 365, 1)
ON CONFLICT (name) DO NOTHING;
