-- 0498 身份型 sink：火花消耗口（产品体检 2026-11-18 A6 / 10-26 P2 挂账）。
--
-- 高净值用户火花沉淀死钱：商店既有身份 SKU（头衔/改名/彩虹ID/头像框）之外，
-- 缺一个「看得见的身份型大额消耗口」。本迁移插一条「发布者之光」头衔 SKU：
--   · 高价（200,000 火花）纯身份消耗，无功能权益——防通胀的身份 sink
--   · 兑付走既有 title_unlock 效果臂（shop.rs:133 has_usable_voucher 判定
--     + 头衔解锁链），零新代码
--   · NOT EXISTS 幂等防重插（0418 抽卡券同款写法）
INSERT INTO shop_items (name, kind, price, config)
SELECT '发布者之光 · 限定头衔（身份纪念）', 'custom_title', 200000,
       jsonb_build_object('title', '发布者之光', 'stackable', false)
WHERE NOT EXISTS (
    SELECT 1 FROM shop_items
    WHERE kind = 'custom_title'
      AND config->>'title' = '发布者之光'
);
