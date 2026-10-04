-- 0279：商店实跑价目对齐 NP 物价水准（0278 的姊妹篇——对齐的是真消费的
-- shop_items 表，不是无人消费的 site_settings 兼容键）
--
-- 背景：0278 核实发现真实商店价与 NP 官方价差 6-50 倍不等（1GB 上传 10x、
-- 邀请 50x），而火花时薪体系两边同量级（本站 cap 150/h vs NP 渐近 ~107/h），
-- 即「赚得差不多、花得贵一个数量级」——上传量/邀请/VIP 系统性偏贵，
-- 改名卡反而便宜 10 倍。对齐后物价回归 NP 手感，经济回路（做种赚火花 →
-- 换上传量/邀请）才成立。
--
-- 敞口：shop_orders 现有 3 笔均为慈善捐赠（价 1000 不变），无历史购买
-- 受调价影响；装扮类（头像框/动态头像）本站自有 SKU、NP 无对应，不动。
-- 已购 uniq 道具的拥有关系不受价格影响（shop_effects 按拥有判定，不按价）。
--
-- 价目来源：NP settings.default.php L228-235 + BonusLogs.php 常量（0278 调研）。

BEGIN;

UPDATE shop_items SET price = 300    WHERE kind = 'upload_credit' AND config->>'gb' = '1'   AND active;   -- 3000→300（NP onegbupload）
UPDATE shop_items SET price = 800    WHERE kind = 'upload_credit' AND config->>'gb' = '5'   AND active;   -- 12000→800（NP fivegbupload）
UPDATE shop_items SET price = 1300   WHERE kind = 'upload_credit' AND config->>'gb' = '10'  AND active;   -- 20000→1300（NP tengbupload）
UPDATE shop_items SET price = 10000  WHERE kind = 'upload_credit' AND config->>'gb' = '100' AND active;   -- 150000→10000（NP hundredgbupload）
UPDATE shop_items SET price = 1000   WHERE kind = 'invite'       AND active;   -- 50000→1000（NP oneinvite）
UPDATE shop_items SET price = 500    WHERE kind = 'temp_invite'  AND active;   -- 20000→500（NP DEFAULT_BONUS_BUY_TEMPORARY_INVITE）
UPDATE shop_items SET price = 100000 WHERE kind = 'rename_card'  AND active;   -- 10000→100000（NP 改名卡高价=改ID是重操作）
UPDATE shop_items SET price = 5000   WHERE kind = 'rainbow_id'   AND active;   -- 30000→5000（NP DEFAULT_BONUS_BUY_RAINBOW）
UPDATE shop_items SET price = 5000   WHERE kind = 'custom_title' AND active;   -- 80000→5000（NP customtitle）
UPDATE shop_items SET price = 8000   WHERE kind = 'vip'          AND active;   -- 100000→8000/28天（NP vipstatus）
UPDATE shop_items SET price = 1000   WHERE kind = 'makeup_card'  AND active;   -- 5000→1000（NP 补签卡）

COMMIT;
