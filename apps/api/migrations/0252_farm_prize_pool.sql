-- 农场收获也能抽「站内虚拟物品」了（0252）。
--
-- 农场和另三个玩法不同：它的收获是**确定性**的（作物表 × 市场窗口），
-- 不该被改成抽奖。所以这里加的不是「收获本身」，而是收获之上的**额外一档**：
-- 收成一株，另外按权重决定要不要多给一点魔力或一件物品。
--
-- 经济口径：作物表按「产量 = 种子价 × 0.75」标定，含 20% 双倍后基础回收率 0.90，
-- 留给额外奖池的预算只有 0.10。单位取**最便宜作物的种子价**（farm_crops 现值），
-- 因为额外档按种子价倍数派彩，越便宜的作物越容易被高档位冲破 1。
-- 这条余量由写侧闸与运行时共同把关（`games::validate_farm`），配超了就拒。

BEGIN;

INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
SELECT 'farm_default', 'farm', '农场 · 收获彩蛋',
       COALESCE((SELECT min(seed_price) FROM farm_crops), 1), true, 40
WHERE NOT EXISTS (SELECT 1 FROM arcade_pools WHERE key = 'farm_default');

-- 播种成一档「什么都不加」：搬家当天行为一字不差，
-- 站长要发物品就在这一档旁边加权重，而不是替换掉收获本身。
DELETE FROM arcade_pool_entries WHERE pool_key = 'farm_default';
INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort)
VALUES ('farm_default', '本轮无额外奖励', 1000, 'magic', 0, 'any', true, 10);

-- 落库断言：基础 0.90 + 表上那一注必须仍 < 1（播种只有一档 0 倍率，天然通过；
-- 谁把播种改肥了，这条就会当场炸给部署的人看）
DO $$
DECLARE
    unit bigint;
    total numeric;
BEGIN
    SELECT min(seed_price) INTO unit FROM farm_crops;
    IF unit IS NULL OR unit <= 0 THEN
        RAISE EXCEPTION 'farm_crops 没有可用种子价，农场彩蛋池无法定标';
    END IF;
    -- 与 games::pool_ev 同一条算式：Σ(权重 × 等值千分) / Σ权重 / (定标 × 1000)。
    -- 魔力位等值 = 定标 × 倍率，物品位等值 = anchor × 件数（目录停用则视为空头承诺）。
    SELECT 0.90 + sum(
                 e.weight * CASE WHEN e.kind = 'magic'
                                 THEN unit * e.mult_permille
                                 ELSE i.anchor * e.qty * 1000
                            END)::numeric
             / nullif(sum(e.weight), 0) / (unit * 1000)
      INTO total
      FROM arcade_pool_entries e
      LEFT JOIN arcade_items i
             ON i.key = e.item_key AND i.enabled
     WHERE e.pool_key = 'farm_default' AND e.enabled;
    IF total IS NULL THEN
        RAISE EXCEPTION '农场彩蛋池没有启用档位：无法核算总回收';
    END IF;
    IF total >= 1 THEN
        RAISE EXCEPTION '农场总回收 % >= 1：额外奖池吃穿了 0.10 的余量', total;
    END IF;
    UPDATE arcade_pools SET ticket = unit WHERE key = 'farm_default';
    RAISE NOTICE '农场彩蛋池就位：定标单位 % 魔力（最便宜种子价），总回收 %',
        unit, total;
END $$;

COMMIT;
