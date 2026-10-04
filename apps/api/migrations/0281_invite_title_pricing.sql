-- 0281：邀请/头衔定价运营口径微调（0280 复审）
--
-- 站长定调（2026-10-04）：
--   · 邀请 = 正式会员名额，就该高定价维持稀缺（临时邀请 5000 供日常流通）；
--     0280 把它修到 10000 是按「结构失衡」算的，但忽略了邀请的准入属性。
--   · 自定义头衔 10000 太便宜——头衔是站内身份象征，低价人人可得就不稀罕了。
--
-- 落位：
--   invite       10000 → 50000（恢复高定价：约 17 天满勤做种，配得起准入门槛）
--   temp_invite  5000 → 5000（不动：日常流通位）
--   custom_title 10000 → 80000（与贵宾月费同档的身份级价格）
--
-- 敞口：shop_orders 仍只有慈善捐赠 3 笔，无历史购买受影响。

BEGIN;

UPDATE shop_items SET price = 50000 WHERE kind = 'invite'       AND active;
UPDATE shop_items SET price = 80000 WHERE kind = 'custom_title' AND active;

COMMIT;
