-- 0460: 时效类 SKU 的「多少天」收口——贵宾待遇缺省 7 天 + 三种时效档配齐 days。
--
-- 背景（2026-11-04 用户反馈）：商店「贵宾待遇」没写多少天（0004 起 config
-- 就是 {}，生效链兜底 30 天，但 0280 把价定到 80000 是月卡口径的价配了
-- 隐形 30 天，买家全凭猜）；「临时邀请」的有效期同样没写（券保质期写死
-- user_vouchers 表默认 interval '30 days'，站长无处可改）。
--
-- 修法（两层）：
--   1) 生效链（crates/flux-shared lib.rs）：vip 缺省 30→7；temp_invite 券
--      保质期改读 config.days（clamp 1..=365，缺省 30，与表默认同口径）。
--      站长覆盖正路 = 道具管理编辑 SKU 的 config.days（vip.days /
--      temp_invite.days 结构化字段本批已加），价格本就可在同一表单改。
--   2) 本迁移把存量种子配齐显式 days：让「库里是什么」与「买家看到什么」
--      一致——不改行为（vip 缺省已由代码改为 7），只把口径落进数据，
--      目录端点回传 config 后前台就能展示天数徽标。
--      ad_free 顺带补显式 15（与生效链缺省、档名「15 天去广告」一致），
--      app_vip 补显式 30（档名自带 30 天口径）。
-- 幂等：jsonb_set 无条件盖写 days，重复执行结果不变。

-- 贵宾待遇：档名同步写明 7 天（买家第一眼）；价 80000 维持（站长可改）
UPDATE shop_items
   SET name = '贵宾待遇(7天)',
       config = jsonb_set(COALESCE(config, '{}'::jsonb), '{days}', '7')
 WHERE kind = 'vip' AND active;

-- APP 专属 VIP：档名已是 30 天口径，只补显式 days
UPDATE shop_items
   SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{days}', '30')
 WHERE kind = 'app_vip' AND active;

-- 去广告：档名「15 天去广告」，补显式 days 对齐
UPDATE shop_items
   SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{days}', '15')
 WHERE kind = 'ad_free' AND active;

-- 临时邀请：券保质期缺省 30 天（核销后邀请码 TTL 另走 invite_admin_ttl_days）
UPDATE shop_items
   SET config = jsonb_set(COALESCE(config, '{}'::jsonb), '{days}', '30')
 WHERE kind = 'temp_invite' AND active;
