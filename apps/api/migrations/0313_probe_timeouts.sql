-- 0313（探测链超时可配化，2026-10-08 硬编码审计 P1）
-- 依据：探测超时 3s/8s 写死在 probe_loop——开源后出厂值人人可查，
-- 作弊端可精确卡线（拖到超时边缘让真探针误判）。进 site_settings
-- 与 probe_jitter_secs/probe_piece_ratio 同组同纪律（0309）。
-- 消费方：guard_refresh::refresh_probe_cfg → probe_cfg::ProbeCfg。
-- 幂等，重跑安全。

INSERT INTO site_settings (name, value, descr, grp)
VALUES
  ('probe_timeout_secs', '3',
   '回连探测 TCP/BT 握手超时（秒）。出厂默认 3 是公开值，建议调整。',
   'anticheat'),
  ('probe_piece_timeout_secs', '8',
   'piece 级抽查超时（秒，含握手+bitfield+unchoke+传输）。'
   '出厂默认 8 是公开值。',
   'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('probe_timeout_secs', 'number', '握手探测超时', 'Probe timeout (secs)',
   'TCP/BT 握手探测的超时。调大减少慢链路误判 SUSPECT，调小加快轮转。'
   '开源默认值公开可查，上线后应调整。',
   '秒', 'anticheat', 30, true, 1, 30, 1),
  ('probe_piece_timeout_secs', 'number', 'piece 抽查超时',
   'Piece probe timeout (secs)',
   'piece SHA-1 抽查全流程超时。大 piece（4MiB）慢链路可能需要 >8s。',
   '秒', 'anticheat', 31, true, 2, 60, 1)
ON CONFLICT (name) DO NOTHING;

-- ============ docs_url（/about 页文档外链可配）============
-- 硬编码审计 P1：docs_url 此前写死作者私人 wiki，每个部署站的外链
-- 都指向它。进 site_settings，缺行回落项目 README（不再是私人域）。
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('docs_url', '',
        '站点文档/帮助页地址（空 = 回落上游项目 README）。/about 页'
        '「文档」链接与页尾兜底都读这里，部署后可指向自己的帮助页。',
        'basic')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('docs_url', 'text', '文档地址', 'Docs URL',
   '站点文档/帮助页的完整 URL（https://…）。留空则 /about 页「文档」链接'
   '指向上游项目 README。',
   '', 'basic', 99, true, NULL, NULL, NULL)
ON CONFLICT (name) DO NOTHING;
