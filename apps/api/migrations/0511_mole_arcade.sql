-- 打地鼠（2026-11-21）：九宫格（jgg）升级为「30 秒敲击局」。
--
-- 与既有六玩法不同，打地鼠**不是一局一抽**，而是「一局多次冒头」：
--   · start  —— 扣票价、由服务端生成整局排程（时间/洞位/档位）并落库；
--   · hit    —— 每次敲击立刻回传，服务端按**收到时刻**判是否在冒头窗口；
--   · finish —— 只结算服务端记录下来的命中，客户端报的分数不作数。
--
-- 经济口径（关键）：奖池在这里是「每次冒头」的档位池，一局冒
-- `mole_pool_spawns` = 12 次（另 2 次是炸弹）。所以闸不能只看
-- 单次 EV < 1，还要看**满盘全中**：每档价值 × 12 必须 < 票价的 95%。
-- 出厂池：单次 EV 0.077，满盘 0.924 —— 手速打满也回不了本，没有增发口。
--
-- 为什么结果在 start 定死：与钓鱼同理，敲击只判时机，不再掷骰子；
-- 客户端拿到排程也无法「只挑大奖」（满盘上限已经压住收益）。

BEGIN;

-- ── 一局一行：排程落库，敲击另表累计 ──────────────────────────────────
CREATE TABLE IF NOT EXISTS arcade_mole_rounds (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id     bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    bet         bigint NOT NULL,
    schedule    jsonb NOT NULL,           -- [{seq,at,up,hole,idx,label,value}]
    idem        text,                     -- start 幂等键：重放不重扣票
    resolved    boolean NOT NULL DEFAULT false,
    score       bigint,                   -- finish 后：命中合计（可为负）
    payout      bigint,                   -- finish 后：实际派彩（>= 0）
    created_at  timestamptz NOT NULL DEFAULT now(),
    resolved_at timestamptz,
    CONSTRAINT arcade_mole_rounds_bet_positive CHECK (bet > 0),
    CONSTRAINT arcade_mole_rounds_idem_unique UNIQUE (idem)
);

CREATE INDEX IF NOT EXISTS idx_arcade_mole_user
    ON arcade_mole_rounds (user_id, created_at DESC);

-- 命中逐条落：主键 (round_id, seq) 本身就是「同一只不能敲两次」的闸
CREATE TABLE IF NOT EXISTS arcade_mole_hits (
    round_id   bigint NOT NULL
        REFERENCES arcade_mole_rounds (id) ON DELETE CASCADE,
    seq        integer NOT NULL,
    value      bigint NOT NULL,
    hit_at     timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (round_id, seq)
);

COMMENT ON TABLE arcade_mole_rounds IS
    '打地鼠一局一行的服务端排程；结算只认 arcade_mole_hits';
COMMENT ON TABLE arcade_mole_hits IS
    '打地鼠逐锤命中：窗口校验在写入前完成，主键防重复';

-- ── 奖池：权重合计 1000，单次 EV 0.077，满盘 0.924 ───────────────────────
INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES ('mole_default', 'mole', '打地鼠 · 标准洞阵', 100, true, 55)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries WHERE pool_key = 'mole_default';

INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort,
     rarity)
VALUES
    ('mole_default', '谢谢参与', 590, 'magic',    0, 'any', true, 10, 1),
    ('mole_default', '灰鼠',     200, 'magic',   50, 'any', true, 20, 1),
    ('mole_default', '蓝耳鼠',   100, 'magic',  100, 'any', true, 30, 2),
    ('mole_default', '粉猪鼠',    55, 'magic',  200, 'any', true, 40, 3),
    ('mole_default', '白兔鼠',    30, 'magic',  500, 'any', true, 50, 4),
    ('mole_default', '墨镜酷鼠',  20, 'magic',  800, 'any', true, 60, 4),
    ('mole_default', '金鼠大奖',   5, 'magic', 3000, 'any', true, 70, 5);

-- 落库断言：权重 1000 + 满盘全中 < 票价 × 95%
DO $$
DECLARE
    w       bigint;
    ev      numeric;
    ceiling numeric;
BEGIN
    SELECT sum(weight) INTO w FROM arcade_pool_entries
     WHERE pool_key = 'mole_default' AND enabled;
    IF w IS DISTINCT FROM 1000 THEN
        RAISE EXCEPTION 'mole_default 权重合计 %，不是 1000', w;
    END IF;
    SELECT sum(weight * mult_permille)::numeric / 1000 / 1000 INTO ev
      FROM arcade_pool_entries
     WHERE pool_key = 'mole_default' AND enabled;
    -- 一局 12 次奖池冒头（炸弹另算，只扣不加）
    ceiling := ev * 12;
    IF ev >= 1 THEN
        RAISE EXCEPTION 'mole_default 单次 EV % >= 1', ev;
    END IF;
    IF ceiling >= 0.95 THEN
        RAISE EXCEPTION
            'mole_default 满盘全中 % x 票价，超过 95%%：会增发', ceiling;
    END IF;
    RAISE NOTICE 'mole_default 就绪：权重 %，单次 EV %，满盘 %',
        w, ev, ceiling;
END $$;

-- ── 大厅：九宫格下架，打地鼠占它的位次 ─────────────────────────────────
INSERT INTO arcade_games
    (key, title_key, icon, href, tone, grp, module_gate, badge, hot,
     enabled, sort)
VALUES
    ('mole', 'mole', '🐹', '/games/mole', 'violet', 'instant', '', 'mole',
     true, true, 30)
ON CONFLICT (key) DO UPDATE SET
    title_key = EXCLUDED.title_key,
    icon = EXCLUDED.icon,
    href = EXCLUDED.href,
    tone = EXCLUDED.tone,
    grp = EXCLUDED.grp,
    module_gate = EXCLUDED.module_gate,
    badge = EXCLUDED.badge,
    hot = EXCLUDED.hot,
    enabled = EXCLUDED.enabled,
    sort = EXCLUDED.sort,
    updated_at = now();

UPDATE arcade_games SET enabled = false, updated_at = now()
 WHERE key = 'jgg';

COMMIT;
