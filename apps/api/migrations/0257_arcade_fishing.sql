-- 钓鱼（计时小游戏）· 2026-09-30
--
-- 设计：**服务端权威 + 结果在抛竿那一刻定死**。
--   · cast  —— 扣鱼饵（=注额）、抽定这一竿的档位、掷一个「多久咬钩」；
--   · reel  —— 只在「咬钩后窗口内」起竿才算中，超时/提前起竿 = 跑空（鱼饵白扣）。
--
-- 为什么结果在 cast 定而不是 reel 定：起竿只判**时机**，不该再掷一次骰子 ——
-- 否则「先看时机、系统再决定给不给」会让人怀疑时机根本没用。定在 cast 上，
-- 起竿就纯粹是手速/反应，规则一眼可解释。
--
-- 玩法仍是**回收口**：奖池 EV < 1，且起竿失败（跑空）会额外吃掉鱼饵 →
-- 玩家实际回收率低于池子的名义值，不会出现增发。

BEGIN;

-- ── 一竿一条：抛竿即落库，起竿读回判时机 ──────────────────────────────
CREATE TABLE IF NOT EXISTS arcade_fishing_rounds (
    id            bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id       bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    bet           bigint NOT NULL,
    entry_index   integer NOT NULL,   -- cast 时定下的档位下标（reel 时按池复原）
    bite_after_ms integer NOT NULL,   -- 抛出后多久咬钩
    window_ms     integer NOT NULL,   -- 咬钩后的起竿窗口
    idem          text,               -- cast 的幂等键：重放不重开一竿
    resolved      boolean NOT NULL DEFAULT false,
    won           boolean,            -- resolved 后有值
    created_at    timestamptz NOT NULL DEFAULT now(),
    resolved_at   timestamptz,
    CONSTRAINT arcade_fishing_rounds_bet_positive CHECK (bet > 0),
    CONSTRAINT arcade_fishing_rounds_idem_unique UNIQUE (idem)
);

CREATE INDEX IF NOT EXISTS idx_arcade_fishing_user
    ON arcade_fishing_rounds (user_id, created_at DESC);

-- ── 奖池：权重合计 1000，EV = 0.858（回收口） ──────────────────────────
INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES ('fishing_default', 'fishing', '钓鱼 · 标准塘', 100, true, 60)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries WHERE pool_key = 'fishing_default';

INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort, rarity)
VALUES
    ('fishing_default', '空钩',    380, 'magic',    0, 'any', true, 10, 1),
    ('fishing_default', '小鱼',    380, 'magic', 1100, 'any', true, 20, 2),
    ('fishing_default', '中鱼',    180, 'magic', 1500, 'any', true, 30, 3),
    ('fishing_default', '大鱼',     50, 'magic', 2400, 'any', true, 40, 4),
    ('fishing_default', '稀有鱼',   10, 'magic', 5000, 'any', true, 50, 5);

DO $$
DECLARE
    w  bigint;
    ev numeric;
BEGIN
    SELECT sum(weight) INTO w FROM arcade_pool_entries
     WHERE pool_key = 'fishing_default' AND enabled;
    IF w IS DISTINCT FROM 1000 THEN
        RAISE EXCEPTION 'fishing_default 权重合计 %，不是 1000', w;
    END IF;
    SELECT sum(weight * mult_permille)::numeric / 1000 / 1000 INTO ev
      FROM arcade_pool_entries WHERE pool_key = 'fishing_default' AND enabled;
    IF ev >= 1 THEN
        RAISE EXCEPTION 'fishing EV % >= 1：在增发', ev;
    END IF;
    RAISE NOTICE '钓鱼就绪：权重 %，EV %', w, ev;
END $$;

COMMIT;
