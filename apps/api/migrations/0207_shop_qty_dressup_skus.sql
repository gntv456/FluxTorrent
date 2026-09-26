-- 0207 商店购买数量 + 装扮多 SKU（用户反馈 2026-09-25）：
-- 1) 可叠加类商品放开数量购买（upload_credit/invite/temp_invite/voucher_*/
--    vip/app_vip/ad_free/charity/rename_card/makeup_card/gift_spark）
-- 2) 装扮类商品按「候选」卖：每枚头像框一件 SKU（avatar_frames 表为货品源），
--    动态头像拆两件固定 SKU（霓虹脉冲/像素星环）。购买页先选款式再下单。
-- 存量兼容：user_dressups/shop_orders 当前为空表，无历史归属要迁移。

-- ---- 装扮 SKU 拆分 ----
-- 头像框（id=15「头像框」占位 SKU）：每框一件，价格随框（price=0 的框卖 2000 底价）
INSERT INTO shop_items (name, kind, price, config)
SELECT '头像框 · ' || f.name, 'avatar_frame',
       CASE WHEN f.price > 0 THEN f.price::bigint ELSE 2000 END,
       jsonb_build_object(
         'dressup', true, 'slot', 'avatar', 'frame_id', f.id
       )
FROM avatar_frames f
WHERE NOT EXISTS (
  SELECT 1 FROM shop_items si
  WHERE si.kind = 'avatar_frame' AND si.config->>'frame_id' = f.id::text
);

-- 动态头像（id=17「动态头像」占位 SKU）：两件固定款式（effect 供前端区分展示，
-- 图片 URL 由管理员后台按需补进 config.avatar_url）
INSERT INTO shop_items (name, kind, price, config) VALUES
  ('动态头像 · 霓虹脉冲', 'animated_avatar', 25000,
   '{"dressup": true, "slot": "avatar", "effect": "neon_pulse"}'::jsonb),
  ('动态头像 · 像素星环', 'animated_avatar', 18000,
   '{"dressup": true, "slot": "avatar", "effect": "pixel_ring"}'::jsonb)
ON CONFLICT DO NOTHING;

-- 旧占位 SKU 退役（不物理删：shop_orders 外键）
UPDATE shop_items SET active = FALSE
WHERE id IN (15, 17)
  AND kind IN ('avatar_frame', 'animated_avatar')
  AND NOT EXISTS (SELECT 1 FROM shop_orders WHERE item_id IN (15, 17));

-- ---- 数量语义标记 ----
-- stackable=true 的 kind 允许 /shop/buy 传 qty>1；装扮/头衔类缺省不可叠加
UPDATE shop_items SET config = config || '{"stackable": true}'::jsonb
WHERE kind IN ('upload_credit', 'invite', 'temp_invite', 'voucher_free',
               'voucher_neutral', 'vip', 'app_vip', 'ad_free', 'charity',
               'rename_card', 'makeup_card', 'gift_spark');
