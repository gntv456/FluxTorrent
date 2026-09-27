-- 0230：G31-D4（编号避让并行会话 0227 captcha） 流水归档策略可配（读路径扩展批）
--
-- 现状：三张 RANGE 月分区流水表（traffic_ledger/spark_ledger/posts）只有
--   「预建未来分区」的 ensure_partitions，没有「过期分区处置」——大站跑两
--   年后 24 个月分区全量在线，查询计划与索引体积持续劣化；且归档窗口写死
--   才能改代码。
-- 处理：站点设定 `ledger_retain_months`（缺省 0 = 永久保留，兼容现状；大站
--   建议 12~24）。worker 的 ensure_partitions 扩展为「预建 + 按保留期
--   DETACH 过期分区（下周期 DROP）」——先 DETACH 使查询立即不再命中、
--   秒级完成且可 ATTACH 回滚；DROP 延后一个周期给误配窗口留缓冲。
--
-- ⚠️ settings_meta.name 有 FK → site_settings.name：必须先插设置行再插
--   元数据行（迁移 0230 首版顺序颠倒曾致启动失败）。

INSERT INTO site_settings (name, value, descr)
VALUES ('ledger_retain_months', '0',
        '流水保留月数（0=永久；设 12~24 时 DROP 超期分区）')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, min, max, group_key, card_order,
   visible)
VALUES
  ('ledger_retain_months', 'int', '流水保留月数',
   'Ledger retention (months)',
   '流量/魔力/帖子流水表的分区保留窗口；0=永久保留（缺省）。设 12~24 时，'
   '超出窗口的旧分区会被移出并删除（对账快照已聚合，不受影响；设小前先备份）。',
   0, 120, 'ops', 61, true)
ON CONFLICT (name) DO NOTHING;
