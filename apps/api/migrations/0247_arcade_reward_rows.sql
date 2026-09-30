-- 确定侧奖励落行表（产品口径：所有数据与奖品站长可配；周常/赛季也是奖品的一部分，
-- 却还写死在 arcade_cfg.rs 的常量里，界面上改不了 —— 配置面在随机侧补齐之后，
-- 确定侧就是最后一块「只能看不能改」的地方）。
--
-- 同时把「确定侧能发物品」这条路开出来：EV 闸管不到这一侧，所以每一笔确定发放
-- 都必须能被预算闸看见。物品按 arcade_items.anchor 折算进预算，
-- 魔力按 spark_ledger 计，两条合起来才是「这一周站点确定付出了多少」。
--
-- 赛季 key 从代码常量改成设置键（缺省仍为 S1）：换赛季是运营动作，不该改代码重发。
INSERT INTO site_settings (name, value, grp, descr)
VALUES ('arcade_season_key', 'S1', 'games',
        '娱乐屋赛季 key：里程碑按此归档，换赛季改这里')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS arcade_quests (
    code         text PRIMARY KEY,
    game_ref     text    NOT NULL,                 -- ref_type；'*' = 任意玩法
    target       integer NOT NULL CHECK (target > 0),
    reward_spark bigint  NOT NULL DEFAULT 0 CHECK (reward_spark >= 0),
    item_key     text REFERENCES arcade_items (key) ON DELETE RESTRICT,
    item_qty     integer NOT NULL DEFAULT 1 CHECK (item_qty > 0),
    enabled      boolean NOT NULL DEFAULT true,
    sort         integer NOT NULL DEFAULT 0,
    -- 一行奖励什么都不发 = 领完什么都没拿到，比报错更难查
    CONSTRAINT arcade_quests_awards_something
        CHECK (reward_spark > 0 OR item_key IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS arcade_milestones (
    code         text PRIMARY KEY,
    season_key   text    NOT NULL,
    need         integer NOT NULL CHECK (need > 0),
    reward_spark bigint  NOT NULL DEFAULT 0 CHECK (reward_spark >= 0),
    item_key     text REFERENCES arcade_items (key) ON DELETE RESTRICT,
    item_qty     integer NOT NULL DEFAULT 1 CHECK (item_qty > 0),
    enabled      boolean NOT NULL DEFAULT true,
    sort         integer NOT NULL DEFAULT 0,
    CONSTRAINT arcade_milestones_awards_something
        CHECK (reward_spark > 0 OR item_key IS NOT NULL)
);

-- ── 逐行照抄 arcade_cfg.rs 的常量播种（切换前后行为必须一字不差）──────────
INSERT INTO arcade_quests (code, game_ref, target, reward_spark, sort) VALUES
    ('q_scratch', 'scratch',    12, 100, 10),
    ('q_bs',      'bigsmall',   10, 150, 20),
    ('q_farm',    'farm_water',  3, 100, 30),
    ('q_week',    '*',          30, 200, 40)
ON CONFLICT (code) DO NOTHING;

INSERT INTO arcade_milestones
    (code, season_key, need, reward_spark, item_key, item_qty, sort) VALUES
    ('m1', 'S1',  3,  300, NULL,    1, 10),
    ('m2', 'S1',  6,  600, NULL,    1, 20),
    ('m3', 'S1',  9,  900, 'board', 1, 30),   -- 零负债外观：确定侧发它不动预算
    ('m4', 'S1', 12, 1500, 'ticket', 1, 40)    -- 有折算价的券：900 计入预算
ON CONFLICT (code) DO NOTHING;

-- ── 落库断言：奖励行引用的物品必须真的发得出去 ────────────────────────────
-- 领取时如果物品停用/缺目录项，确定侧没有「回落价」可退（回落就是按 anchor 折魔力，
-- anchor 为 0 就等于把奖励吞掉），所以这种引用在库里就不该存在。
DO $$
DECLARE
    bad bigint;
BEGIN
    SELECT count(*) INTO bad
      FROM (
        SELECT item_key FROM arcade_quests    WHERE enabled AND item_key IS NOT NULL
        UNION ALL
        SELECT item_key FROM arcade_milestones WHERE enabled AND item_key IS NOT NULL
      ) r
     WHERE NOT EXISTS (SELECT 1 FROM arcade_items i
                        WHERE i.key = r.item_key AND i.enabled);
    IF bad > 0 THEN
        RAISE EXCEPTION '% 条确定侧奖励引用了停用/不存在的物品', bad;
    END IF;
END $$;
