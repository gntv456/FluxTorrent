-- 娱乐屋新增两玩法奖池：扭蛋机（capsule）+ 大转盘（wheel）（2026-09-30）。
--
-- 两者与九宫格/刮刮乐**同构**：都是「票价 + 加权池抽档」，因此完全复用
-- `load_pool` + `draw_entry` + `settle`，不引入任何新机制 —— 这里只播种行表。
-- 奖池名不与既有模块「抽卡(gacha)」冲突：扭蛋是胶囊机（出站内物品），
-- 抽卡是独立 gacha 模块（卡牌图鉴），两条链路各自独立。
--
-- EV 一律 < 1（回收口纪律），落库用 DO 块断言，写错在迁移阶段就炸掉。

BEGIN;

INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES
    ('capsule_default', 'capsule', '扭蛋机 · 标准池', 100, true, 40),
    ('wheel_default',   'wheel',   '大转盘 · 标准池', 100, true, 50)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries
 WHERE pool_key IN ('capsule_default', 'wheel_default');

-- 扭蛋机：权重合计 1000，EV = 0.855
INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort)
VALUES
    ('capsule_default', '谢谢参与', 700, 'magic',     0, 'any', true, 10),
    ('capsule_default', '1.2x 魔力', 150, 'magic',  1200, 'any', true, 20),
    ('capsule_default', '2x 魔力',   90, 'magic',  2000, 'any', true, 30),
    ('capsule_default', '5x 魔力',   45, 'magic',  5000, 'any', true, 40),
    ('capsule_default', '10x 魔力',  12, 'magic', 10000, 'any', true, 50),
    ('capsule_default', '50x 大奖',   3, 'magic', 50000, 'any', true, 60);

-- 大转盘：权重合计 1000，EV = 0.95
INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort)
VALUES
    ('wheel_default', '谢谢参与', 620, 'magic',     0, 'any', true, 10),
    ('wheel_default', '1.5x 魔力', 200, 'magic',  1500, 'any', true, 20),
    ('wheel_default', '2x 魔力',   100, 'magic',  2000, 'any', true, 30),
    ('wheel_default', '3x 魔力',    50, 'magic',  3000, 'any', true, 40),
    ('wheel_default', '8x 魔力',    25, 'magic',  8000, 'any', true, 50),
    ('wheel_default', '20x 大奖',    5, 'magic', 20000, 'any', true, 60);

-- 落库断言：权重合计 1000，且 EV < 1
DO $$
DECLARE
    p     text;
    w     bigint;
    ev    numeric;
BEGIN
    FOREACH p IN ARRAY ARRAY['capsule_default', 'wheel_default'] LOOP
        SELECT sum(weight) INTO w FROM arcade_pool_entries
         WHERE pool_key = p AND enabled;
        IF w IS DISTINCT FROM 1000 THEN
            RAISE EXCEPTION '% 权重合计 %，不是 1000', p, w;
        END IF;
        SELECT sum(weight * mult_permille)::numeric / 1000 / 1000 INTO ev
          FROM arcade_pool_entries WHERE pool_key = p AND enabled;
        IF ev >= 1 THEN
            RAISE EXCEPTION '% EV % >= 1：在增发，拒绝该配置落库', p, ev;
        END IF;
        RAISE NOTICE '% 行表化完成：权重 %，EV %', p, w, ev;
    END LOOP;
END $$;

COMMIT;
