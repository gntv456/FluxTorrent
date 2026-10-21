-- 0414：给「慈善捐赠」档位补上发放金额（P-476）
--
-- 0004_economy_seed 里这行的 config 一直是 '{}'，而生效链的慈善分支要求
-- 先从 config 算出 >0 的金额才落池（`if amount > 0`）。算不出就**静默跳过**：
-- 会员的 1000 火花在购买里被扣掉，pool_donations 没有行、magic_pool 不进账、
-- 接口照样回 200。10-21 现网实测：root 名下三笔慈善订单（id 33/35/49，各付
-- 1000），每笔下单后五分钟内的进池记录都是空。
--
-- 口径：「花多少捐多少」——把档位自身的售价补成发放额。已经显式写了金额键
-- （spark / sparks / amount / price）的行不动，避免覆盖站方手工设的面值。
-- 幂等：条件里排掉已有键，重复执行 0 行受影响。
UPDATE shop_items
   SET config = config || jsonb_build_object('spark', price)
 WHERE kind = 'charity'
   AND NOT (
        config ? 'spark'
     OR config ? 'sparks'
     OR config ? 'amount'
     OR config ? 'price'
   );
