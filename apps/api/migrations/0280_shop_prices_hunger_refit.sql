-- 0280：商店价目「饥饿感」定稿——在 0279 的 NP 结构基准上乘回 ×10 折价系数
--
-- 背景：0279 照搬 NP 名义价后复查发现口径错位——购买力=时薪×时长，两边
-- 火花/魔力时薪同量级（本站 cap 150/h 渐近 vs NP ~107/h 渐近），而本站
-- 0279 前的定价整体就是 NP 的 ~10 倍（原价其实自洽）；直接用 NP 名义价
-- = 全场打一折，做种两小时就能买 1GB 上传，「消耗>产出」的饥饿感没了。
-- 定稿口径：**结构用 NP 的（同商品间相对比例），绝对水位保持本站折价
-- 系数 ×10**，并修掉原定价里失衡的两端（邀请 50x、改名卡 0.1x）。
--
-- 敞口：shop_orders 仍只有慈善捐赠 3 笔（1000 不动），无历史购买受影响。

BEGIN;

UPDATE shop_items SET price = 3000   WHERE kind = 'upload_credit' AND config->>'gb' = '1'   AND active;  -- 维持原价（1GB=3000）
UPDATE shop_items SET price = 8000   WHERE kind = 'upload_credit' AND config->>'gb' = '5'   AND active;  -- 12000→8000（线性对齐）
UPDATE shop_items SET price = 13000  WHERE kind = 'upload_credit' AND config->>'gb' = '10'  AND active;  -- 20000→13000
UPDATE shop_items SET price = 100000 WHERE kind = 'upload_credit' AND config->>'gb' = '100' AND active;  -- 150000→100000
UPDATE shop_items SET price = 10000  WHERE kind = 'invite'        AND active;  -- 50000→10000（50x 失衡修正）
UPDATE shop_items SET price = 5000   WHERE kind = 'temp_invite'   AND active;  -- 20000→5000
UPDATE shop_items SET price = 100000 WHERE kind = 'rename_card'   AND active;  -- 维持 100000（改名是重操作，高价=冷静期）
UPDATE shop_items SET price = 15000  WHERE kind = 'rainbow_id'    AND active;  -- 30000→15000（装饰类小溢价）
UPDATE shop_items SET price = 10000  WHERE kind = 'custom_title'  AND active;  -- 80000→10000（16x 失衡修正）
UPDATE shop_items SET price = 80000  WHERE kind = 'vip'           AND active;  -- 100000→80000（身份类维持稀缺）
-- makeup_card 1000（0279 已落）：日常小额消耗品就该便宜，不动

COMMIT;
