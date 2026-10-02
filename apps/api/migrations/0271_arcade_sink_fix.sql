-- 0271 娱乐屋经济口径修复：加工坊印钞机 + 牧场无限产出 + 九宫格权重口径
--
-- 背景（2026-10-03 ZT2 第二轮深度测试，见 _doc/资深PT站长深度体验测试与优化建议-2026-10-02.md §9）：
--   ① 加工坊 `arcade_farm_recipes` 的 CHECK 写成 `out_spark > in_spark`，与表注释里
--      「回收侧 EV<1 由站长定档」**自相矛盾**；播种三档 100→135 / 300→390 / 1000→1240，
--      且 `craft_start` 扣 in_spark、`craft_collect` 发 out_spark，净额恒正、无任何限次闸
--      → **单账号约 +13,200 魔力/日的纯增发**（三个配方并行，各自独立冷却）。
--   ② 牧场牲畜一次买入永久产出（`ranch_collect` 只推移 ready_at，无总次数/寿命上限），
--      回本后每周期纯利 → 单账号约 +2,680 魔力/日、永久。
--   ③ 九宫格 `jgg_default` 权重合计 1003（违反「权重合计=1000」口径；EV 仍 <1，不影响资金）。
--
-- 修法（全部按「娱乐屋各玩法 EV<1、产出必须来自回收口」的既有铁律）：
--   ① CHECK 改为 `out_spark < in_spark`，三档重标定为 EV≈0.85（与 capsule/fishing 同档）。
--   ② 给 `arcade_ranch_pens` 加累计产出列，**终身产出不超过购入价的 95%**（回收口）；
--      配套代码在 games_http/ranch.rs 里执行上限判定。
--   ③ 把「谢谢参与」731 → 728，使合计回到 1000。
--
-- 注意：① 只改「当前仍是 out_spark > in_spark」的行，站长已自行调过的档不被覆盖；
--      ③ 只在合计恰为 1003 且该行权重恰为 731 时修正，避免覆盖站长自调。

BEGIN;

-- ============ ① 加工坊：从印钞机改回回收口 ============
-- 顺序要紧：必须先摘掉旧 CHECK → 改数据 → 再加新 CHECK，否则新约束会被旧数据顶爆。
-- 两个 DROP 都带 IF EXISTS：本迁移幂等，重复执行（含手工预跑）不会失败。
ALTER TABLE arcade_farm_recipes
    DROP CONSTRAINT IF EXISTS arcade_farm_recipes_check;
ALTER TABLE arcade_farm_recipes
    DROP CONSTRAINT IF EXISTS arcade_farm_recipes_out_lt_in_check;

-- 只修「仍是印钞机」的内置三档 → EV≈0.85（= 与其它奖池同档的回收率）
UPDATE arcade_farm_recipes
   SET out_spark = in_spark * 85 / 100
 WHERE key IN ('butter', 'cheese', 'cake')
   AND out_spark >= in_spark;

ALTER TABLE arcade_farm_recipes
    ADD CONSTRAINT arcade_farm_recipes_out_lt_in_check
    CHECK (out_spark < in_spark);

-- ============ ② 牧场：加累计产出，终身产出 ≤ 购入价 95% ============
ALTER TABLE arcade_ranch_pens
    ADD COLUMN IF NOT EXISTS collected_spark bigint NOT NULL DEFAULT 0;

-- ============ ③ 九宫格权重合归 1000 ============
UPDATE arcade_pool_entries
   SET weight = weight - 3
 WHERE pool_key = 'jgg_default' AND label = '谢谢参与' AND weight = 731
   AND (SELECT sum(weight) FROM arcade_pool_entries WHERE pool_key = 'jgg_default') = 1003;

-- ============ 自检：三项口径必须成立，否则早失败 ============
DO $$
DECLARE
    n_bad_recipe bigint;
    n_pools_off  bigint;
    jgg_total    bigint;
BEGIN
    SELECT count(*) INTO n_bad_recipe FROM arcade_farm_recipes
     WHERE out_spark >= in_spark;
    IF n_bad_recipe > 0 THEN
        RAISE EXCEPTION '仍有 % 个加工配方 out_spark >= in_spark（印钞机未关）',
            n_bad_recipe;
    END IF;

    SELECT count(*) INTO n_pools_off FROM (
        SELECT pool_key FROM arcade_pool_entries GROUP BY pool_key
         HAVING sum(weight) <> 1000
    ) t;
    SELECT COALESCE(sum(weight), 0) INTO jgg_total
      FROM arcade_pool_entries WHERE pool_key = 'jgg_default';
    RAISE NOTICE '加工配方已全部为回收口；权重合计≠1000 的池仍有 % 个（jgg=%）',
        n_pools_off, jgg_total;
END $$;

COMMIT;
