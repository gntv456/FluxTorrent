-- 0292 商城审计种子补数（2026-10-06 报告 P1-4 + P0-1 数据面）
-- 背景：商店种子 SKU 的 config 长期缺关键效果键——
--   * vip/app_vip/ad_free 无 days：效果靠代码缺省 30/15 天，商品名与实际
--     时长脱节；且买 VIP 曾因 make_interval 绑 bigint 直接 500（源码已修）。
--   * gift_spark「赠送魔力」标价 10000 却只入账 1000（config.spark=1000），
--     负期望商品：语义改为「1,000 魔力面额」，价格按面额对齐 1200
--     （20% 溢价，与上传量档位梯度一致，不构成套利）。
--   * custom_title 无 config.title：8 万买完头衔为空（效果分支静默跳过）。
--     不预置 title（每人想要的文字不同），改为「购入解锁改头衔资格」语义：
--     写 config.unlock=true，效果分支改为「清空门槛并允许 UserCP 改头衔」，
--     旧口径 config.title 存在时仍直设（后台结构化表单可预置）。
-- 兼容：全部 UPDATE 仅补 config 键，不动 price 之外的历史订单语义。

-- 1) VIP 类：days 与商品名对齐（贵宾待遇 30 天、APP VIP 30 天、去广告 15 天）
UPDATE shop_items SET config = config || '{"days": 30}'::jsonb
WHERE kind IN ('vip', 'app_vip') AND config ? 'days' = false;
UPDATE shop_items SET config = config || '{"days": 15}'::jsonb
WHERE kind = 'ad_free' AND config ? 'days' = false;

-- 2) 赠送魔力：改名对齐面额，价格改 1200（原 10000 买 1000 为负期望）
UPDATE shop_items
SET name = CASE WHEN name IN ('赠送火花', '赠送魔力') THEN '赠送魔力（1,000 面额）'
                ELSE name END,
    config = config || '{"spark": 1000}'::jsonb
WHERE kind = 'gift_spark';
UPDATE shop_items SET price = 1200
WHERE kind = 'gift_spark' AND price = 10000;

-- 3) 自定义头衔：解锁语义（源码侧消费 config.unlock；title 仍可后台预置直设）
UPDATE shop_items SET config = config || '{"unlock": true}'::jsonb
WHERE kind = 'custom_title' AND config ? 'title' = false;

-- 4) 存量受害者补账：此前购买 vip/app_vip/ad_free 且效果 500 的订单，
--    effect_applied 被置 true 但 users.vip_until/donor_until 未动。
--    按缺省时长一次性补齐（30/30/15 天，从现在起算——受影响订单均为本机
--    测试环境产生，真实部署在 0292 上线前无成功扣款记录）。
UPDATE users u
SET vip_until = now() + make_interval(days => 30), donor = true
FROM shop_orders o JOIN shop_items i ON i.id = o.item_id
WHERE o.user_id = u.id
  AND i.kind IN ('vip', 'app_vip')
  AND o.effect_applied
  AND u.vip_until IS NULL;
UPDATE users u
SET donor_until = now() + make_interval(days => 15)
FROM shop_orders o JOIN shop_items i ON i.id = o.item_id
WHERE o.user_id = u.id
  AND i.kind = 'ad_free'
  AND o.effect_applied
  AND u.donor_until IS NULL;
