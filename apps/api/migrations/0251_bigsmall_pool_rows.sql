-- 猜大小也读奖池行表（0251）。
--
-- 前四刀的最后一案：刮刮乐与九宫格已经读 arcade_pools，猜大小的赔率还住在
-- `games_bigsmall_win_mult` 设置键上 —— 那条路上同样没有 EV 闸、没有跨池回查，
-- 而且它和「奖品是站内虚拟物品」这条定调不搭：猜中只能返魔力，不能给一件东西。
--
-- 形状上它确实和前两个玩法不同：结果取决于玩家猜哪一侧，所以档位要标明
-- 「哪一侧付」。加一列 side（win | tie | lose | any），权重仍是**一整局的千分占比**
-- （win 区合计 490 + tie 20 + lose 490 = 1000），这样 `weight` 这一列在三个玩法里
-- 是同一把尺，EV 公式也仍是那一份：Σ(权重×价值)/Σ权重/票价。
--
-- 平局返本、猜错归零这些是**机制**，留在代码里；本表配的是「各区按什么付、付多少」，
-- 也就是站长能改的那一层。

BEGIN;

ALTER TABLE arcade_pool_entries
    ADD COLUMN IF NOT EXISTS side text NOT NULL DEFAULT 'any';
ALTER TABLE arcade_pool_entries DROP CONSTRAINT IF EXISTS arcade_pool_entries_side_known;
ALTER TABLE arcade_pool_entries ADD CONSTRAINT arcade_pool_entries_side_known
    CHECK (side IN ('any', 'win', 'tie', 'lose'));

INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES ('bigsmall_default', 'bigsmall', '猜大小 · 标准桌', 1, true, 30)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries WHERE pool_key = 'bigsmall_default';

-- 赔率取当前设置键现值（停读之前派生一次），平局区按机制返本 = 1000‰
INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort)
VALUES
    ('bigsmall_default', '猜中派彩', 490, 'magic',
     -- 派生自现存的倍数设置键（1.9 → 1900‰），搬家前后同一张桌
     COALESCE((SELECT round(value::numeric * 1000) FROM site_settings
                WHERE name = 'games_bigsmall_win_mult'), 1900)::bigint,
     'win', true, 10),
    ('bigsmall_default', '平局返本',  20, 'magic', 1000, 'tie',  true, 20),
    ('bigsmall_default', '猜错归零', 490, 'magic',    0, 'lose', true, 30);

-- 停读即删键：留着就是设置面板里一个「改了没反应」的假开关
DELETE FROM settings_meta WHERE name = 'games_bigsmall_win_mult';
DELETE FROM site_settings WHERE name = 'games_bigsmall_win_mult';

-- 落库断言：三区的千分占比必须铺满 1000，且 EV 必须仍是回收口（< 1）。
-- 0.951 这个数是猜大小的经济口径，搬家前后必须一模一样。
DO $$
DECLARE
    w     bigint;
    ev    numeric;
    miss  bigint;
BEGIN
    SELECT sum(weight) INTO w FROM arcade_pool_entries
     WHERE pool_key = 'bigsmall_default' AND enabled;
    IF w IS DISTINCT FROM 1000 THEN
        RAISE EXCEPTION '猜大小三区权重合计 %，不是 1000', w;
    END IF;

    SELECT count(*) INTO miss FROM (
        SELECT side, sum(weight) AS s FROM arcade_pool_entries
         WHERE pool_key = 'bigsmall_default' AND enabled GROUP BY side
    ) r WHERE r.s IS DISTINCT FROM CASE r.side
              WHEN 'win' THEN 490 WHEN 'tie' THEN 20 ELSE 490 END;
    IF miss > 0 THEN
        RAISE EXCEPTION '% 个区的权重不符合 49/2/49 机制', miss;
    END IF;

    SELECT sum(weight * mult_permille)::numeric / 1000 / 1000 INTO ev
      FROM arcade_pool_entries
     WHERE pool_key = 'bigsmall_default' AND enabled;
    IF ev >= 1 THEN
        RAISE EXCEPTION '猜大小 EV % >= 1：在增发，拒绝该配置落库', ev;
    END IF;
    IF ev <> 0.951 THEN
        RAISE EXCEPTION '猜大小 EV 应为 0.951（1.9x / 49-2-49），实为 %', ev;
    END IF;
    RAISE NOTICE '猜大小行表化：三区合计 %，EV %', w, ev;
END $$;

COMMIT;
