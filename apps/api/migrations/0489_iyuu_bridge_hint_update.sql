-- 0489: IYUU 辅种面板设置项文案补运营账（0486 的 follow-up，只动
-- settings_meta 的 hint，不动值、不动结构）。
--
-- 背景：iyuu_bridge_batch 只说了「不要调太大」，没告诉站长**换算关系**。
-- 现网 13k 已过审种、每小时一轮、缺省每轮 30 条 = 每天 720 次查询，
-- 全表刷一轮需要约 18 天。调大 batch = 全周期线性缩短，但 IYUU 配额
-- 消耗同速加快——这个权衡必须写在设置项里，站长才调得有依据。
--
-- 同批把 sites_extra 的 hint 补一句「覆盖对本站自身不生效」（自站命中
-- 已在出口过滤，2026-11-12），免得站长以为要靠它屏蔽自站。
--
-- 幂等：settings_meta ON CONFLICT DO UPDATE（字段声明随代码走，重跑安全）。

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options,
                           group_key, card_order, secret) VALUES
('iyuu_bridge_batch', 'int', '每轮刷新条数',
 'Batch per run',
 '查重由 worker 每小时批量跑，这里限制每轮最多查多少条（缺省 30）。'
 || '换算：每天最多查 24×该值 条——13k 种按缺省 30 ≈ 18 天刷完一轮；'
 || '调大 batch 全周期线性缩短，但 IYUU 配额消耗同速加快（配额制，'
 || '超额后全部查询失败）。有命中的行会优先复核（死链窗口 2~3 天），'
 || '没命中的行晚点刷不影响面板。填 0 = 暂停刷新但保留配置',
 NULL, '插件', 5, false),
('iyuu_bridge_sites_extra', 'textarea', '站点表覆盖',
 'Site table overrides',
 '站点表默认每天从 IYUU 自动同步（约 125 站）。这里只用于'
 || '改名 / 换域名 / 改详情页路径，或同步失败时手动补：'
 || '每行 `sid|站点名|域名`（如 `140|新站|new.example`），'
 || '优先级最高、改完立即生效。出现 `#数字` 说明三层表都没这个 id。'
 || '本站自身的命中已在面板出口自动过滤，无需在此处理',
 NULL, '插件', 4, false)
ON CONFLICT (name) DO UPDATE
  SET hint = EXCLUDED.hint;
