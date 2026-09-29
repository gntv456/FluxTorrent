-- 娱乐玩法奖池行表化第一步（口径见 _doc 策划案与样张 .workbuddy/games-mock/）。
-- 目标：把「参数在行表」这条纪律落到九宫格奖池上 —— 现在它是 apps/api/src/games/jgg.rs:13
-- 的 `pub const JGG_PRIZES`，管理端改不动，EV 单测锁的也是一个没人能改的常量。
--
-- 本文件**只建表 + 原样播种**，不改任何 Rust 调用点：`JGG_PRIZES` 仍是运行时唯一生效的表，
-- 因此本迁移落地后行为零变化（可安全回滚）。下一刀才把 casino/overview/arcade_admin
-- 三个读 const 的点切到本表，并把校验改成「不合法就关闸拒绝服务」，
-- 而不是像 scratch.rs 的 from_parts 那样静默回落 DEFAULT。
--
-- 命名跟随 0240 已确立的 arcade_* 前缀（arcade_claims），不另立 play_* 第二套命名。

-- 奖池：一个玩法下的若干档池（九宫格目前一池；刮刮乐是三档票价 = 三池）
CREATE TABLE IF NOT EXISTS arcade_pools (
    key         text PRIMARY KEY,
    game        text NOT NULL,               -- 'jgg' | 'scratch' | 'bigsmall' | 'farm'
    label       text NOT NULL,
    ticket      bigint NOT NULL,             -- 票价（魔力）
    enabled     boolean NOT NULL DEFAULT true,
    sort        integer NOT NULL DEFAULT 0,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT arcade_pools_ticket_positive CHECK (ticket > 0)
);

CREATE INDEX IF NOT EXISTS idx_arcade_pools_game ON arcade_pools (game) WHERE enabled;

-- 奖池条目：权重 + 赔付。payout 是**相对票价的倍数**（与 JggPrize.payout 同口径），
-- 因此 EV = Σ(权重×倍数)/Σ权重，恒等于 games/tests.rs 现在锁的那个 0.725。
-- 物品位/收藏位留给下一刀的 kind/item_key；本刀只搬魔力位，避免一次改两层。
CREATE TABLE IF NOT EXISTS arcade_pool_entries (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    pool_key    text NOT NULL REFERENCES arcade_pools (key) ON DELETE CASCADE,
    label       text NOT NULL,
    weight      integer NOT NULL,
    payout      bigint NOT NULL,             -- 倍数；0 = 不中
    enabled     boolean NOT NULL DEFAULT true,
    sort        integer NOT NULL DEFAULT 0,
    CONSTRAINT arcade_pool_entries_weight_positive CHECK (weight > 0),
    -- 权重为 0 的档位永远抽不到，是「配了但没生效」的经典假配置，直接拒
    CONSTRAINT arcade_pool_entries_payout_nonneg CHECK (payout >= 0)
);

CREATE INDEX IF NOT EXISTS idx_arcade_pool_entries_pool ON arcade_pool_entries (pool_key) WHERE enabled;

-- ── 播种：逐行照抄 apps/api/src/games/jgg.rs 的 JGG_PRIZES，一个字都不改 ──
-- 权重合计 1000；EV = (120×1 + 60×2 + 50×3 + 25×5 + 10×10 + 3×20 + 1×50) / 1000 = 0.725
INSERT INTO arcade_pools (key, game, label, ticket, sort) VALUES
    ('jgg_default', 'jgg', '九宫格 · 标准池', 100, 0)
ON CONFLICT (key) DO NOTHING;

INSERT INTO arcade_pool_entries (pool_key, label, weight, payout, sort)
SELECT * FROM (VALUES
    ('jgg_default', '谢谢参与', 731,  0::bigint, 0),
    ('jgg_default', '再来一次', 120,  1::bigint, 1),
    ('jgg_default', '2x 魔力',   60,  2::bigint, 2),
    ('jgg_default', '3x 魔力',   50,  3::bigint, 3),
    ('jgg_default', '5x 魔力',   25,  5::bigint, 4),
    ('jgg_default', '10x 魔力',  10, 10::bigint, 5),
    ('jgg_default', '20x 魔力',   3, 20::bigint, 6),
    ('jgg_default', '50x 魔力',   1, 50::bigint, 7)
) AS v(pool_key, label, weight, payout, sort)
WHERE NOT EXISTS (SELECT 1 FROM arcade_pool_entries e WHERE e.pool_key = 'jgg_default');

-- 落库即校验：EV 必须 < 1（回收口纪律），且权重合计必须与 const 一致。
-- 这里用 DO 块抛错，让「播种写错」在迁移阶段就炸掉，而不是留一张坏表给运行时。
DO $$
DECLARE
    total  bigint;
    ev     numeric;
BEGIN
    SELECT SUM(weight), SUM(weight * payout)::numeric / NULLIF(SUM(weight), 0)
      INTO total, ev
      FROM arcade_pool_entries
     WHERE pool_key = 'jgg_default' AND enabled;

    IF total IS DISTINCT FROM 1000 THEN
        RAISE EXCEPTION 'jgg_default 权重合计应为 1000，实得 %', total;
    END IF;
    IF ev IS DISTINCT FROM 0.725 THEN
        RAISE EXCEPTION 'jgg_default EV 应为 0.725（回收口），实得 %', ev;
    END IF;
END $$;
