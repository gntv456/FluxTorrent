-- 让物品位真正「在用」：0244 只是把能力建起来，jgg_default 里仍然一个物品档都没有，
-- 于是「玩法抽各种物品」这条产品口径还没落地。本文件把顶部两档纯魔力换成物品档。
--
-- 换档后 EV 必须仍然 < 1，且**必须按 arcade_items.anchor 算**（不是按登记价、
-- 也不是按 payout 倍数），所以断言里 JOIN 目录取 anchor。
-- 魔力位的 value = 票价 × 倍数；物品位的 value = anchor × 件数。

BEGIN;

DELETE FROM arcade_pool_entries
 WHERE pool_key = 'jgg_default' AND label IN ('20x 魔力', '50x 魔力');

INSERT INTO arcade_pool_entries (pool_key, label, weight, payout, kind, item_key, qty, sort) VALUES
    ('jgg_default', '抽卡券 ×1',        4, 0, 'item', 'ticket',   1,  8),
    ('jgg_default', '补签卡',            2, 0, 'item', 'makeup',   1,  9),
    ('jgg_default', '免考核卡 · 3 天',   1, 0, 'item', 'pass3',    1, 10);

-- 落库断言：EV（含物品按 anchor 折算）必须 < 1；票价取池上的 ticket。
DO $$
DECLARE
    t        bigint;
    total    bigint;
    ev       numeric;
    missing  bigint;
BEGIN
    SELECT ticket INTO t FROM arcade_pools WHERE key = 'jgg_default';
    IF t IS NULL OR t <= 0 THEN
        RAISE EXCEPTION 'jgg_default 票价必须为正，实为 %', t;
    END IF;

    -- 物品位必须能在目录里找到启用且有折算价的物品，否则是空头承诺
    SELECT count(*) INTO missing
      FROM arcade_pool_entries e
     WHERE e.pool_key = 'jgg_default' AND e.enabled AND e.kind = 'item'
       AND (e.item_key IS NULL
            OR NOT EXISTS (SELECT 1 FROM arcade_items i
                            WHERE i.key = e.item_key AND i.enabled AND i.anchor > 0));
    IF missing > 0 THEN
        RAISE EXCEPTION '% 个物品位引用了不存在/停用/无折算价的物品', missing;
    END IF;

    SELECT SUM(e.weight),
           SUM(e.weight * (CASE WHEN e.kind = 'item'
                                THEN (SELECT i.anchor * e.qty FROM arcade_items i WHERE i.key = e.item_key)
                                ELSE t * e.payout END))::numeric / SUM(e.weight) / t
      INTO total, ev
      FROM arcade_pool_entries e
     WHERE e.pool_key = 'jgg_default' AND e.enabled;

    IF total IS NULL OR total <= 0 THEN
        RAISE EXCEPTION 'jgg_default 权重合计非法: %', total;
    END IF;
    IF ev >= 1 THEN
        RAISE EXCEPTION 'jgg_default 含物品综合返还 % >= 1：玩法在增发，拒绝该配置落库', ev;
    END IF;

    RAISE NOTICE 'jgg_default 换档后：权重合计 %，含物品综合返还 %', total, round(ev, 4);
END $$;

COMMIT;
