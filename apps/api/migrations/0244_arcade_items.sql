-- 娱乐屋物品目录与发放账（产品口径：玩法在消耗魔力的同时要能抽出「各种物品」，
-- 且物品与奖池一律后台可配）。本文件只建能力，**不改 jgg_default 的现有行**：
-- 物品位要和「代码真的会发物品」同批上线，否则中间态会出现一行 payout=0 的物品档，
-- 老加载器把它当成 0 倍魔力奖，标签读起来是骗人的。

-- ── 物品目录 ──────────────────────────────────────────────────────────────
-- kind 用自由 TEXT 而不是若干个布尔列：新增一类奖励不该改表结构。
-- anchor 是**权威折算价值**（魔力等值），EV 一律按它算，不按登记价 ——
-- 登记价可以被改小，anchor 不行；anchor 必须能说出来源（anchor_src）。
CREATE TABLE IF NOT EXISTS arcade_items (
    key          text PRIMARY KEY,
    name         text NOT NULL,
    kind         text NOT NULL,                    -- economic | voucher | cosmetic
    anchor       bigint NOT NULL DEFAULT 0,        -- 魔力等值；cosmetic 为 0（零负债）
    anchor_src   text NOT NULL DEFAULT 'derived',  -- shop | derived | declared | n/a
    unlimited    boolean NOT NULL DEFAULT true,    -- 与 stock 分开：0 不双关「无限」和「耗尽」
    stock        bigint NOT NULL DEFAULT 0,        -- 仅 unlimited=false 时生效：全服余量
    per_user     integer NOT NULL DEFAULT 1,       -- 每人可得上限
    icon         text NOT NULL DEFAULT '',
    enabled      boolean NOT NULL DEFAULT true,
    sort         integer NOT NULL DEFAULT 0,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    -- 「不限量」和「限量但余量为 0」是两件不同的事，混在一列上会把无界当耗尽、
    -- 抬高回落 EV（样张阶段踩过这个坑，落库直接用两列表达）
    CONSTRAINT arcade_items_stock_shape CHECK (unlimited OR stock >= 0),
    CONSTRAINT arcade_items_per_user_positive CHECK (per_user > 0)
);

-- ── 发放账 ────────────────────────────────────────────────────────────────
-- side 是这套设计的关键：随机侧（rand）受奖池 EV 闸管，确定侧（det，周常/赛季）
-- **绕得过 EV 闸**，所以必须单独能被统计出来去对发放预算。没有这一列，
-- 「周常送免考核卡」就是一条没人拦得住的侧门。
CREATE TABLE IF NOT EXISTS arcade_item_grants (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    item_key    text NOT NULL REFERENCES arcade_items (key) ON DELETE CASCADE,
    user_id     bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    qty         integer NOT NULL DEFAULT 1,
    game        text NOT NULL,
    side        text NOT NULL DEFAULT 'rand',      -- rand | det
    idem        text,                              -- 发放幂等键；重放不重发
    granted_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT arcade_item_grants_idem_unique UNIQUE (idem),
    CONSTRAINT arcade_item_grants_qty_positive CHECK (qty > 0),
    CONSTRAINT arcade_item_grants_side_known CHECK (side IN ('rand', 'det'))
);

CREATE INDEX IF NOT EXISTS idx_arcade_grants_user_item
    ON arcade_item_grants (user_id, item_key);
CREATE INDEX IF NOT EXISTS idx_arcade_grants_side_time
    ON arcade_item_grants (side, granted_at);

-- 全服余量：限量物品按发放账反推，不另存一份「已用数」（第二份清单必然漂移）
CREATE OR REPLACE VIEW arcade_item_stock_left AS
    SELECT i.key,
           i.unlimited,
           CASE WHEN i.unlimited THEN NULL::bigint
                ELSE i.stock - COALESCE((SELECT SUM(g.qty)::bigint
                                           FROM arcade_item_grants g
                                          WHERE g.item_key = i.key), 0)
           END AS left
      FROM arcade_items i;

-- ── 奖池条目：支持物品位 ──────────────────────────────────────────────────
ALTER TABLE arcade_pool_entries ADD COLUMN IF NOT EXISTS kind     text NOT NULL DEFAULT 'magic';
ALTER TABLE arcade_pool_entries ADD COLUMN IF NOT EXISTS item_key text REFERENCES arcade_items (key) ON DELETE SET NULL;
ALTER TABLE arcade_pool_entries ADD COLUMN IF NOT EXISTS qty      integer NOT NULL DEFAULT 1;

-- magic 位用 payout（票价倍数）；item 位 payout 必须为 0，价值只从 arcade_items.anchor 派生。
-- 这条约束把「物品位偷偷填个 payout」和「magic 位挂个 item_key」都在库侧堵掉。
ALTER TABLE arcade_pool_entries DROP CONSTRAINT IF EXISTS arcade_pool_entries_kind_shape;
ALTER TABLE arcade_pool_entries ADD CONSTRAINT arcade_pool_entries_kind_shape CHECK (
    (kind = 'magic' AND item_key IS NULL)
    OR (kind = 'item' AND item_key IS NOT NULL AND payout = 0)
);

-- ── 播种物品目录（与样张 .workbuddy/games-mock/arcade-shared.js 的 ITEMS 同口径）──
-- anchor 全部标出来源；declared 的是运营自估，公示页必须标注且需显式确认。
INSERT INTO arcade_items (key, name, kind, anchor, anchor_src, unlimited, stock, per_user, icon, sort) VALUES
    ('pass3',    '免考核卡 · 3 天',      'economic', 12000, 'derived', false,    6, 1, '🛡', 10),
    ('makeup',   '补签卡',               'economic',  1200, 'shop',    false,   30, 2, '📅', 20),
    ('bank500',  '银行券 500',           'economic',   500, 'shop',     true,    0, 20, '🏦', 30),
    ('ticket',   '抽卡券 ×1',            'voucher',    900, 'derived',  false,   40, 5, '🎟', 40),
    ('frame',    '卡框 · 鎏金',          'cosmetic',     0, 'n/a',      true,    0, 1, '🖼', 50),
    ('board',    '板面 · 电路板',        'cosmetic',     0, 'n/a',      true,    0, 1, '⌨', 60),
    ('title',    '称号 · 夜游神',        'cosmetic',     0, 'n/a',      true,    0, 1, '🏷', 70),
    ('showcase', '主页展示权 · 30 天',   'cosmetic',     0, 'n/a',      true,    0, 1, '✨', 80)
ON CONFLICT (key) DO NOTHING;

-- ── 落库断言：目录本身不能自相矛盾 ────────────────────────────────────────
DO $$
DECLARE
    bad_econ bigint;
    bad_src  bigint;
BEGIN
    -- 经济类物品必须有正折算价，否则「零负债」是假的
    SELECT count(*) INTO bad_econ FROM arcade_items
     WHERE kind = 'economic' AND anchor <= 0;
    IF bad_econ > 0 THEN
        RAISE EXCEPTION '% 个 economic 物品 anchor<=0：零负债标记不可信', bad_econ;
    END IF;
    -- 声称按商店价 JOIN 的，商店里必须真有这件（否则 anchor 是手填的）
    SELECT count(*) INTO bad_src FROM arcade_items
     WHERE anchor_src NOT IN ('shop', 'derived', 'declared', 'n/a');
    IF bad_src > 0 THEN
        RAISE EXCEPTION '% 个物品 anchor_src 取值非法', bad_src;
    END IF;
END $$;
