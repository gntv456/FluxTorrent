-- 0418 抽卡券商店 SKU（二轮深测 B2，P1）：抽卡券此前只有签到
-- daily_free 一条获取路径（每天 1 张，攒一抽要 25 天），商店无 SKU，
-- module_gacha=yes 的站对普通用户等于橱窗。
--
-- 定价口径（0280「饥饿感」批同源）：单抽票价 25，10 连 250。
-- 魔力时薪 ~10-150，抽卡券单价 500 ≈ 1-2 小时做种收入一抽，
-- 与商店其它消耗品（补签卡 1000/免费券 2500）同梯度偏入门，
-- 不冲击签到福利（签到 27 天 ≈ 27 券免费，商店买是赶时间选项）。
--
-- 幂等：NOT EXISTS 防重插，重跑安全。

INSERT INTO shop_items (name, kind, price, config)
SELECT '抽卡券（单张）', 'gacha_ticket', 500,
       jsonb_build_object('qty', 1, 'stackable', true)
WHERE NOT EXISTS (
  SELECT 1 FROM shop_items
  WHERE kind = 'gacha_ticket' AND (config->>'qty')::int = 1
);

INSERT INTO shop_items (name, kind, price, config)
SELECT '抽卡券 · 十连包（10 张）', 'gacha_ticket', 4500,
       jsonb_build_object('qty', 10, 'stackable', false)
WHERE NOT EXISTS (
  SELECT 1 FROM shop_items
  WHERE kind = 'gacha_ticket' AND (config->>'qty')::int = 10
);
