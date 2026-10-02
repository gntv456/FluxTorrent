-- 猜大小切三骰机制（样图⑥）：side 取值域从 win/tie/lose 改成
-- win/triple/lose（豹子 = 三颗全同，判负区，可被护盾救）；三区权重
-- 从 490/20/490 改为 486/28/486（105/216、6/216、105/216 取整）。

-- 先把存量 tie 行改判到 triple（bigsmall 标准桌的「平局返本」在新机制
-- 下语义即「豹子判负」；其他玩法池的 any/win 不受影响），再换约束。
UPDATE arcade_pool_entries SET side = 'triple'
 WHERE side = 'tie' AND pool_key IN (
     SELECT key FROM arcade_pools WHERE game = 'bigsmall'
 );

ALTER TABLE arcade_pool_entries
    DROP CONSTRAINT IF EXISTS arcade_pool_entries_side_known;
ALTER TABLE arcade_pool_entries
    ADD CONSTRAINT arcade_pool_entries_side_known
        CHECK (side IN ('any', 'win', 'triple', 'lose'));

-- 标准桌重铺（0251 的三行换成三骰口径；mult 仍读现值——站长改过赔率也保得住）
DELETE FROM arcade_pool_entries WHERE pool_key = 'bigsmall_default';
INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort)
VALUES
    ('bigsmall_default', '猜中派彩', 486, 'magic',
     COALESCE((SELECT round(value::numeric * 1000) FROM site_settings
                WHERE name = 'games_bigsmall_win_mult'), 1900)::bigint,
     'win', true, 10),
    ('bigsmall_default', '豹子判负',  28, 'magic',    0, 'triple', true, 20),
    ('bigsmall_default', '猜错归零', 486, 'magic',    0, 'lose', true, 30);

-- 落库断言：三区合计仍是 1000‰，且 EV 仍是回收口（<1）
DO $$
DECLARE
    w  bigint;
    ev numeric;
BEGIN
    SELECT sum(weight) INTO w FROM arcade_pool_entries
     WHERE pool_key = 'bigsmall_default';
    IF w <> 1000 THEN
        RAISE EXCEPTION 'bigsmall 权重合计 %≠1000', w;
    END IF;
    SELECT sum(weight * mult_permille)::numeric / 1000 / 1000 INTO ev
      FROM arcade_pool_entries WHERE pool_key = 'bigsmall_default';
    IF ev >= 1 THEN RAISE EXCEPTION 'bigsmall EV % ≥ 1（放水）', ev; END IF;
END $$;
