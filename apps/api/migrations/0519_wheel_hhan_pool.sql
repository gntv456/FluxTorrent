-- 0519 大转盘改「憨憨结构」奖池（2026-11-24）。
--
-- 参考 hhanclub.net/lucky.php 的实读奖品权重表（10 档，priority 即权重，
-- 合计 4851；每次消耗 2000 憨豆，魔力项期望回报约 489/次 → 回报率 ≈ 24%）。
-- 憨憨的形状：一大保底安慰档（彩虹 ID 82.5%）+ 稀缺真奖（≤10%）——
-- 是一个比我们现行 wheel（EV≈0.85）重得多的回收口。
--
-- 落地映射（形状照搬，数值按我们站重标）：
--   彩虹 ID（保底）→ 0.15x 安慰档（返 15 魔力）
--   魔力 5000/2000/1000/100 → 2.5x / 1x / 0.5x 魔力
--   VIP 7 天        → 补签卡 ×1   （arcade_items.resub_card，anchor 5000）
--   补签卡          → 抽卡券 ×1   （arcade_items.ticket，anchor 900）
--   上传量 2/5 GB   → 银行券 500 ×1/×2（bank500，anchor 500）
--   邀请 ×1         → 魔力 3x / 10x（邀请券无 SKU 折算价，改魔力档）
--
-- 为什么权重不能照抄：憨憨的高权重档是「彩虹 ID」这类软价值奖品，而我们的
-- 物品位按 anchor 折算真魔力 —— 直接搬原权重，补签卡一档（anchor 5000）就
-- 把 EV 顶过 1（等于增发）。权重按「anchor 反比 + 保底主导」重标，
-- 保底档 0.15x 占 94.9%，EV = 0.413 < 0.5（重回收口；对比现行 0.85）。
--
-- 切换方式：旧池 wheel_default 禁用（不删，站长改两个 enabled 一键回退）；
-- 新池 wheel_hhan 挂 game='wheel'（load_pool 按 game 读唯一 enabled 池）。

BEGIN;

-- ── 1) 新池 ─────────────────────────────────────────────────────────────
INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES ('wheel_hhan', 'wheel', '大转盘 · 憨憨结构池', 100, true, 51)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries WHERE pool_key = 'wheel_hhan';

INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, item_key, qty, side,
     enabled, sort, rarity)
VALUES
    -- 安慰保底档：憨憨彩虹 ID（82.5%）的形状映射
    ('wheel_hhan', '谢谢参与',      4000, 'magic',  150,  NULL, 1, 'any', true, 10, 1),
    -- 魔力档（憨憨 5000/2000/1000 的形状映射）
    ('wheel_hhan', '2.5x 魔力',       60, 'magic', 2500,  NULL, 1, 'any', true, 20, 3),
    ('wheel_hhan', '1x 魔力',         50, 'magic', 1000,  NULL, 1, 'any', true, 30, 2),
    ('wheel_hhan', '0.5x 魔力',       35, 'magic',  500,  NULL, 1, 'any', true, 40, 2),
    -- 物品位：补签卡 / 抽卡券 / 银行券（anchor 折算，权重按价值反比缩）
    ('wheel_hhan', '补签卡 ×1',       10, 'item',    0, 'resub_card', 1, 'any', true, 50, 3),
    ('wheel_hhan', '抽卡券 ×1',       20, 'item',    0, 'ticket',     1, 'any', true, 60, 3),
    ('wheel_hhan', '银行券 500 ×1',   25, 'item',    0, 'bank500',    1, 'any', true, 70, 3),
    ('wheel_hhan', '银行券 500 ×2',    8, 'item',    0, 'bank500',    2, 'any', true, 80, 4),
    -- 大奖（憨憨邀请位的映射：真稀缺）
    ('wheel_hhan', '3x 魔力',          6, 'magic', 3000,  NULL, 1, 'any', true, 90, 4),
    ('wheel_hhan', '10x 大奖',         2, 'magic', 10000, NULL, 1, 'any', true, 100, 5);

-- ── 2) 切换：旧池禁用（保留行，回退 = 改两个 enabled） ──────────────────
UPDATE arcade_pools SET enabled = false, updated_at = now()
 WHERE key = 'wheel_default' AND enabled;

-- ── 3) 落库断言：权重合计 4216 + EV < 0.5（憨憨结构重回收口径） ─────────
DO $$
DECLARE
    w       bigint;
    ev      numeric;
    tkt     numeric;
BEGIN
    SELECT sum(weight) INTO w FROM arcade_pool_entries
     WHERE pool_key = 'wheel_hhan' AND enabled;
    IF w IS DISTINCT FROM 4216 THEN
        RAISE EXCEPTION 'wheel_hhan 权重合计 %，不是 4216', w;
    END IF;

    SELECT ticket INTO tkt FROM arcade_pools WHERE key = 'wheel_hhan';
    -- 与 games::jgg::pool_ev 同构：
    --   magic: value_permille = tkt * mult
    --   item : value_permille = anchor * qty * 1000
    --   EV = Σ(w * value_permille) / (Σw * tkt * 1000)
    SELECT sum(
             CASE WHEN e.kind = 'magic'
                  THEN e.weight * e.mult_permille * tkt
                  ELSE e.weight * COALESCE(i.anchor, 0) * e.qty * 1000
             END
           )::numeric / (w * tkt * 1000)
      INTO ev
      FROM arcade_pool_entries e
      LEFT JOIN arcade_items i ON i.key = e.item_key AND i.enabled
     WHERE e.pool_key = 'wheel_hhan' AND e.enabled;
    IF ev >= 1 THEN
        RAISE EXCEPTION 'wheel_hhan EV % >= 1：会增发，拒绝落库', ev;
    END IF;
    IF ev >= 0.5 THEN
        RAISE EXCEPTION
            'wheel_hhan EV % 超出憨憨结构口径（目标 < 0.5 的重回收口）', ev;
    END IF;
    RAISE NOTICE 'wheel_hhan 就绪：权重 %，EV %（憨憨结构，重回收口）',
        w, ev;
END $$;

COMMIT;
