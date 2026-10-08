-- 0309（开源反作弊威胁模型收口，2026-10-08）
-- 依据：开源 = 检测算法公开。对策不是藏代码（藏不住：黑盒可探测），
-- 而是「算法公开、密钥私有」：把可以随站变化的部分挪进 site_settings，
-- 让每个站的实际探测行为不可从上游源码预测。
--
-- 本迁移落三件：
-- ① probe_jitter_secs：探测循环的启动抖动（0-本值 秒的随机延迟）。
--    现状：探测任务固定 300s tick，首次探测必然出现在进程启动后
--    恰好 300s——时间表是源码常量，作弊客户端可以「卡表」避开探测窗。
--    加抖动后，何时探在每个站、每个重启周期都不同。
-- ② probe_piece_ratio：piece 级抽查的抽样比例（0.0-1.0）。
--    现状：FLUX_TRACKER_PIECE_PROBE 固定取采样集**前 N 个**（idx<N），
--    顺序可预测——作弊者只要让自己的 peer 排在采样序后段就永不被抽查。
--    改为按比例随机抽取后，抽中概率与位置无关。
-- ③ settings hint 更新：反作弊组全部键的 hint 补「出厂默认值公开可查」
--    提示——开源引擎的默认参数是公开知识，上线后必须按本站情况改。
--
-- 全部幂等（ON CONFLICT DO NOTHING/UPDATE），重跑安全。

-- ============ ① 探测调度抖动 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('probe_jitter_secs', '90',
        '回连/piece 探测循环的随机启动延迟上界（秒）。0=不抖动（不推荐：'
        '固定节拍的探测时间表可被客户端预判避开）。开源引擎的探测代码'
        '人人可读，抖动让每个站的实际探测时机不可预测。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('probe_jitter_secs', 'number', '探测时机抖动', 'Probe jitter (secs)',
   '每轮探测前附加 0~本值 秒的随机延迟，防止客户端按固定节拍卡表避开探测。'
   '出厂默认 90s 是公开值，建议改为任意非默认数。',
   '秒', 'anticheat', 19, true, 0, 600, 1)
ON CONFLICT (name) DO NOTHING;

-- ============ ② piece 抽查随机化 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('probe_piece_ratio', '0.25',
        'piece 级抽查的抽样比例（对已通过握手+bitfield 的 peer）：'
        '0=只做握手+bitfield 不抽查 piece；1=全部抽查（出站带宽大，慎用）。'
        '抽查对象按比例随机抽取，与采样序位置无关。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('probe_piece_ratio', 'number', 'piece 抽查比例', 'Piece probe ratio',
   'piece 哈希抽查是唯一能实锤「数据是伪造」的手段，代价是每抽一个 peer '
   '要传一个 piece（16KiB~4MiB 出站）。0.25 = 每轮对 1/4 通过握手的 peer 抽查，'
   '随机选人不卡位置。',
   '比率', 'anticheat', 20, true, 0, 1, 0.05)
ON CONFLICT (name) DO NOTHING;

-- ============ ③ 反作弊组默认值公开提示 ============
-- 开源引擎的出厂默认是公开知识：留着默认值的站，等于把调参插在门上。
-- 只改 hint 不改值——已上线的站有自己的节奏，不搞静默改参。
UPDATE settings_meta
   SET hint = hint ||
     '【开源提示】出厂默认值随源码公开，人人可查；上线后请按本站情况调整。'
 WHERE group_key = 'anticheat'
   AND visible
   AND NOT readonly
   AND hint NOT LIKE '%出厂默认值随源码公开%';
