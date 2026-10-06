-- 0287：道具库存配额（2026-10-06 道具发放管理第五轮实测）
--
-- 背景：shop_items.active 是发放/购买的唯一闸门，没有「本期限量 N 份」。
-- 活动道具靠人工 count(*) 对数，超发只能事后发现。
--
-- 设计：
--   * stock_quota  NULL = 不限量（默认，全存量道具行为不变）
--   * stock_used   已发放/已购买计数（管理员发放与商店购买共用一个池）
--   * 回收/作废回滚计数——库存口径「净持有」而非「累计发出」
--
-- ⚠️ 值行先于登记行（settings_meta.name 有 FK → site_settings.name）。

ALTER TABLE shop_items ADD COLUMN IF NOT EXISTS stock_quota bigint;
ALTER TABLE shop_items ADD COLUMN IF NOT EXISTS stock_used bigint NOT NULL DEFAULT 0;

COMMENT ON COLUMN shop_items.stock_quota IS
    '限量配额（NULL=不限）；管理员发放与购买共用，回收回滚';
COMMENT ON COLUMN shop_items.stock_used IS
    '已消耗配额数；stock_quota 非空时 grant/buy 前检查 stock_used < stock_quota';

-- 现有存量按订单数回填（含 0 价管理发放；回收的负向由代码层继续维护）
UPDATE shop_items i
   SET stock_used = (SELECT count(*) FROM shop_orders o WHERE o.item_id = i.id)
 WHERE stock_used = 0;
