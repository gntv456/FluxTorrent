-- 娱乐屋生态补全（外部对标差距分析 P0/P1，2026-09-30）
--
-- 五件事，全部服务「把玩法串成生态」：
--   1. 对局档位落库（arcade_pool_rounds）：稀有掉落全服播报 / 渔获图鉴 /
--      未来赛季统计共用的唯一事实源 —— 结算时在 settle_tx 顺带写一行。
--   2. 票根册补 4 玩法指标（扭蛋/大转盘/钓鱼/宠物此前在收集面上隐身），
--      集齐票根「站长亲授」阈值 11 → 17。
--   3. 补签卡进奖池物品目录：自助补签（/attendance/resub）同时认
--      商店订单与娱乐屋发放的卡（use_kind 增 'resub'）—— 财神式
--      「游戏产出 → 签到救急」闭环。
--   4. 渔汛：周末活动塘（fishing_event 池，EV 0.93）+ 一竿的 event 标记；
--      门槛沿用行为联动的周做种阈值（games_fishing_event_seed_hours）。
--   5. 鱼竿升级 sink（arcade_fishing_rods）：花魔力买起竿窗口，不碰概率。

BEGIN;

-- ── 1) 对局档位落库 ──────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS arcade_pool_rounds (
    id          bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id     bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    game        text NOT NULL,
    entry_index integer NOT NULL DEFAULT 0,
    rarity      smallint NOT NULL DEFAULT 1,
    prize       text NOT NULL DEFAULT '',
    spark       bigint NOT NULL DEFAULT 0,   -- 魔力派彩（物品位 = 0）
    value       bigint NOT NULL DEFAULT 0,   -- 魔力等值（含物品 anchor / 回落）
    created_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_arcade_rounds_rare
    ON arcade_pool_rounds (created_at DESC) WHERE rarity >= 4;
CREATE INDEX IF NOT EXISTS idx_arcade_rounds_user_game
    ON arcade_pool_rounds (user_id, game);

COMMENT ON TABLE arcade_pool_rounds IS
    '娱乐屋对局档位流水：settle_tx 每局一行，稀有播报(rarity>=4)与渔获图鉴的数据源';

-- ── 2) 票根册：4 玩法指标 + 集齐阈值 ─────────────────────────────────────
INSERT INTO achievement_defs
    (family, code, name, descr, metric, threshold, reward_sparks, position)
VALUES
  ('arcade', 'arc_capsule_20',  '蛋壳收藏家',   '累计抽 20 发扭蛋',
   'capsule_plays', 20,   0, 13),
  ('arcade', 'arc_wheel_50',    '转运的人',     '大转盘累计 50 转',
   'wheel_plays',   50,   0, 14),
  ('arcade', 'arc_fishing_30',  '第一竿',       '累计抛竿 30 次',
   'fishing_plays', 30,   0, 15),
  ('arcade', 'arc_fishing_200', '渔瘾',         '累计抛竿 200 次',
   'fishing_plays', 200,  0, 16),
  ('arcade', 'arc_pet_50',      '铲屎官',       '累计投喂 50 次',
   'pet_feeds',     50,   0, 17),
  ('arcade', 'arc_plays_1000',  '票根册 · 终章', '累计游玩 1000 局',
   'game_plays',    1000, 0, 18)
ON CONFLICT (code) DO NOTHING;

UPDATE achievement_defs
   SET descr = '集齐其余 17 张票根', threshold = 17
 WHERE family = 'arcade' AND code = 'arc_all_star';

-- ── 3) 补签卡：目录 + 用途约束 ───────────────────────────────────────────
ALTER TABLE arcade_items DROP CONSTRAINT IF EXISTS arcade_items_use_kind_known;
ALTER TABLE arcade_items ADD CONSTRAINT arcade_items_use_kind_known
    CHECK (use_kind IN ('collect', 'spark', 'sku', 'game', 'resub'));

ALTER TABLE arcade_item_uses DROP CONSTRAINT IF EXISTS arcade_item_uses_kind_known;
ALTER TABLE arcade_item_uses ADD CONSTRAINT arcade_item_uses_kind_known
    CHECK (use_kind IN ('collect', 'spark', 'sku', 'game', 'resub'));

-- anchor=5000 与商店同款同价（0004 种子 makeup_card 售价 5000），anchor_src=shop
INSERT INTO arcade_items
    (key, name, kind, anchor, anchor_src, unlimited, stock, per_user,
     icon, enabled, sort, use_kind, use_ref)
VALUES ('resub_card', '补签卡', 'voucher', 5000, 'shop', true, 0, 5,
        '🗓️', true, 90, 'resub', '')
ON CONFLICT (key) DO NOTHING;

-- ── 4) 渔汛：活动塘 + 一竿的 event 标记 ─────────────────────────────────
ALTER TABLE arcade_fishing_rounds
    ADD COLUMN IF NOT EXISTS event boolean NOT NULL DEFAULT false;

INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES ('fishing_event', 'fishing_event', '渔汛 · 周末限定塘', 100, true, 61)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries WHERE pool_key = 'fishing_event';

INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort, rarity)
VALUES
    ('fishing_event', '空钩而归', 450, 'magic',     0, 'any', true, 10, 1),
    ('fishing_event', '银鱼',     300, 'magic',   800, 'any', true, 20, 2),
    ('fishing_event', '汛期大鱼', 160, 'magic',  1500, 'any', true, 30, 3),
    ('fishing_event', '洄游鲸鲤',  70, 'magic',  3000, 'any', true, 40, 4),
    ('fishing_event', '渔汛传说',  20, 'magic', 12000, 'any', true, 50, 5);

DO $$
DECLARE
    w  bigint;
    ev numeric;
BEGIN
    SELECT sum(weight) INTO w FROM arcade_pool_entries
     WHERE pool_key = 'fishing_event' AND enabled;
    IF w IS DISTINCT FROM 1000 THEN
        RAISE EXCEPTION 'fishing_event 权重合计 %，不是 1000', w;
    END IF;
    SELECT sum(weight * mult_permille)::numeric / 1000 / 1000 INTO ev
      FROM arcade_pool_entries WHERE pool_key = 'fishing_event' AND enabled;
    IF ev >= 1 THEN
        RAISE EXCEPTION '渔汛 EV % >= 1：在增发', ev;
    END IF;
    RAISE NOTICE '渔汛就绪：权重 1000，EV %', ev;
END $$;

-- ── 5) 鱼竿升级（sink：花魔力买起竿窗口，不改概率） ─────────────────────
CREATE TABLE IF NOT EXISTS arcade_fishing_rods (
    user_id     bigint PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    level       integer NOT NULL DEFAULT 1 CHECK (level BETWEEN 1 AND 5),
    upgraded_at timestamptz NOT NULL DEFAULT now()
);

-- ── 6) 设置键 ────────────────────────────────────────────────────────────
INSERT INTO site_settings (name, value, grp, descr) VALUES
  ('fishing_event_enabled', '1', 'games',
   '渔汛开关（1=周末对本周做种达标者开放活动塘）'),
  ('fishing_rod_window_bonus_ms', '150', 'games',
   '鱼竿每级增加的起竿窗口毫秒数')
ON CONFLICT (name) DO NOTHING;

COMMIT;
