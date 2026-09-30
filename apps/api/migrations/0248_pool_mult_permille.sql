-- 刮刮乐奖池行表化 + 「倍数」改千分比。
--
-- 为什么要动这一刀：0242 建了奖池行表、0245 让九宫格读它，但刮刮乐的概率与倍数
-- 一直住在五个设置键里 —— 那条路上没有 EV 闸、没有跨池回查、也没有物品位可言。
-- 「所有奖品站长随意配置」要覆盖到刮刮乐，它必须先读同一张表。
--
-- 倍数换成千分比（mult_permille）：刮刮乐有 0.5x 档，整数倍数装不下它。
-- 千分比是更通用的一侧（整数倍数 = 千分比 × 1000），所以九宫格的行一起搬，
-- 不留「两列各表一件事」的那种会漂移的形状。
--
-- EV 口径不变、只是算得更准：魔力位价值 = 票价 × 千分比 / 1000；
-- 校验与公示用**不取整**的精确值（对玩家更有利的一侧），实际派彩仍向下取整。

BEGIN;

-- ① 先加新列并把旧值搬过去，**再**删旧列：反过来会把九宫格的倍数直接清零。
ALTER TABLE arcade_pool_entries
    ADD COLUMN IF NOT EXISTS mult_permille bigint NOT NULL DEFAULT 0;

UPDATE arcade_pool_entries
   SET mult_permille = payout * 1000
 WHERE kind = 'magic' AND mult_permille = 0 AND payout <> 0;

DO $$
DECLARE
    moved bigint;
BEGIN
    SELECT count(*) INTO moved FROM arcade_pool_entries
     WHERE kind = 'magic' AND mult_permille > 0;
    IF moved = 0 THEN
        RAISE EXCEPTION '魔力位的倍数没搬进千分比列：中止，别把奖池读空';
    END IF;
END $$;

ALTER TABLE arcade_pool_entries
    DROP CONSTRAINT IF EXISTS arcade_pool_entries_kind_shape;
ALTER TABLE arcade_pool_entries DROP COLUMN IF EXISTS payout;
ALTER TABLE arcade_pool_entries ADD CONSTRAINT arcade_pool_entries_kind_shape CHECK (
    (kind = 'magic' AND item_key IS NULL)
    OR (kind = 'item' AND item_key IS NOT NULL AND mult_permille = 0)
);

-- ② 刮刮乐的表从设置键现值播种（迁移时读一次，之后设置键停读 ——
--    清单只能有一份，第二份必须派生，这里派生的是「切换前那一刻的真值」）。
--    档位规则沿用原实现：前三档 + 空档之外，2x/10x 按 8:2 分余量，
--    十档填了正数且合计正好 100 时按配置。
INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES ('scratch_default', 'scratch', '刮刮乐 · 标准票', 1, true, 20)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries WHERE pool_key = 'scratch_default';

INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, enabled, sort)
WITH cfg AS (
    SELECT
        COALESCE((SELECT value::int FROM site_settings
                   WHERE name = 'games_scratch_empty_pct'), 45) AS e,
        COALESCE((SELECT value::int FROM site_settings
                   WHERE name = 'games_scratch_half_pct'), 30) AS h,
        COALESCE((SELECT value::int FROM site_settings
                   WHERE name = 'games_scratch_one_pct'), 15) AS o,
        COALESCE((SELECT value::int FROM site_settings
                   WHERE name = 'games_scratch_two_pct'), 8) AS t,
        COALESCE((SELECT value::int FROM site_settings
                   WHERE name = 'games_scratch_ten_pct'), 2) AS n
), calc AS (
    SELECT e, h, o, t,
           CASE WHEN n > 0 AND e + h + o + t + n = 100
                THEN n ELSE 100 - e - h - o - t END AS ten
      FROM cfg
)
SELECT 'scratch_default', x.label, x.weight, 'magic', x.mp, true, x.srt
  FROM calc, LATERAL (VALUES
        ('未中奖',   calc.e,       0, 10),
        ('返本一半', calc.h,     500, 20),
        ('返本',     calc.o,    1000, 30),
        ('2 倍',     calc.t,    2000, 40),
        ('10 倍',    calc.ten, 10000, 50)
  ) AS x(label, weight, mp, srt)
 WHERE x.weight > 0;

-- ③ 落库断言：搬过来的刮刮乐表 EV 必须仍然 < 1，且权重合计 100
DO $$
DECLARE
    ev numeric;
    w  bigint;
BEGIN
    SELECT sum(weight),
           sum(weight * mult_permille)::numeric / nullif(sum(weight), 0) / 1000
      INTO w, ev
      FROM arcade_pool_entries
     WHERE pool_key = 'scratch_default' AND enabled;
    IF w IS DISTINCT FROM 100 THEN
        RAISE EXCEPTION '刮刮乐权重合计 %，不是 100', w;
    END IF;
    IF ev >= 1 THEN
        RAISE EXCEPTION '刮刮乐播种后 EV % >= 1：设置键里的现值本身就在增发', ev;
    END IF;
    RAISE NOTICE '刮刮乐行表化：权重合计 %，EV %', w, round(ev, 4);
END $$;

COMMIT;
