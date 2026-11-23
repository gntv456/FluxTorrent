-- 大转盘奖池扩档：6 档 → 18 档（2026-11-21）。
--
-- 背景：样图定稿的转盘是 18 个扇区（十几二十档才有「密盘」质感），
-- 但 0254 只播了 6 档，盘面稀、档位少。这里按样图的档位族重播，
-- 全部用魔力位：物品位要挂 arcade_items 的 anchor，库存/每人上限
-- 会让「转到却发不出」变成常态，盘面档位不该背这层风险。
--
-- 经济口径不变：权重合计仍是 1000，EV = 0.9509 < 1（0254 的 0.95
-- 一档不差地保住），落库用整数比较断言，写错在迁移阶段就炸。
--
-- 档位形状（权重：倍率）：谢谢 519:0 · 0.5x 55 · 1.1x 85 · 1.2x 72
-- 1.5x 107 · 1.8x 40 · 2x 38 · 2.5x 26 · 3x 20 · 4x 12 · 5x 8
-- 6x 6 · 8x 4 · 10x 3 · 12x 2 · 20x/30x/50x 各 1。
-- 标签口径：档名一律「Nx 魔力」字面倍率，不写「返本」——0.5x 实际
-- 只回半注（净输一半），叫返本是文案欺诈；保本线在 1x，本池没有 1x 档。

BEGIN;

-- 0510 会把 6 档替换成 18 档；线上站长可能已自行改过 wheel_default。
-- 覆盖前先留一份原表快照（只留 wheel 池，表已存在则不再重复拍），
-- 出问题可以按 id 恢复到 arcade_pool_entries。
CREATE TABLE IF NOT EXISTS arcade_pool_entries_backup_0510 AS
SELECT * FROM arcade_pool_entries WHERE pool_key = 'wheel_default';

DELETE FROM arcade_pool_entries WHERE pool_key = 'wheel_default';

INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled,
     sort, rarity)
VALUES
    ('wheel_default', '谢谢参与',    519, 'magic',     0, 'any', true,  10, 1),
    ('wheel_default', '0.5x 魔力',    55, 'magic',   500, 'any', true,  20, 2),
    ('wheel_default', '1.1x 魔力',    85, 'magic',  1100, 'any', true,  30, 2),
    ('wheel_default', '1.2x 魔力',    72, 'magic',  1200, 'any', true,  40, 2),
    ('wheel_default', '1.5x 魔力',   107, 'magic',  1500, 'any', true,  50, 3),
    ('wheel_default', '1.8x 魔力',    40, 'magic',  1800, 'any', true,  60, 3),
    ('wheel_default', '2x 魔力',      38, 'magic',  2000, 'any', true,  70, 3),
    ('wheel_default', '2.5x 魔力',    26, 'magic',  2500, 'any', true,  80, 3),
    ('wheel_default', '3x 魔力',      20, 'magic',  3000, 'any', true,  90, 3),
    ('wheel_default', '4x 魔力',      12, 'magic',  4000, 'any', true, 100, 3),
    ('wheel_default', '5x 魔力',       8, 'magic',  5000, 'any', true, 110, 4),
    ('wheel_default', '6x 魔力',       6, 'magic',  6000, 'any', true, 120, 4),
    ('wheel_default', '8x 魔力',       4, 'magic',  8000, 'any', true, 130, 4),
    ('wheel_default', '10x 大奖',      3, 'magic', 10000, 'any', true, 140, 4),
    ('wheel_default', '12x 大奖',      2, 'magic', 12000, 'any', true, 150, 5),
    ('wheel_default', '20x 大奖',      1, 'magic', 20000, 'any', true, 160, 5),
    ('wheel_default', '30x 传奇',      1, 'magic', 30000, 'any', true, 170, 5),
    ('wheel_default', '50x 传奇',      1, 'magic', 50000, 'any', true, 180, 5);

DO $$
DECLARE
    n      bigint;
    w      bigint;
    weighted numeric;
BEGIN
    SELECT count(*), sum(weight) INTO n, w
      FROM arcade_pool_entries
     WHERE pool_key = 'wheel_default' AND enabled;
    IF n <> 18 THEN
        RAISE EXCEPTION '大转盘档位 % 个，样图要求 18', n;
    END IF;
    IF w IS DISTINCT FROM 1000 THEN
        RAISE EXCEPTION '大转盘权重合计 %，不是 1000', w;
    END IF;
    -- 整数口径比浮点稳：Σ(权重 × 千分倍率) < 1000 × 1000 = 不增发
    SELECT sum(weight * mult_permille) INTO weighted
      FROM arcade_pool_entries
     WHERE pool_key = 'wheel_default' AND enabled;
    IF weighted >= 1000 * 1000 THEN
        RAISE EXCEPTION '转盘 EV % ≥ 1：在增发，拒绝该配置落库',
            weighted / 1000 / 1000;
    END IF;
    RAISE NOTICE '大转盘 18 档落库：权重 %，EV %',
        w, round(weighted / 1000 / 1000, 4);
END $$;

COMMIT;
