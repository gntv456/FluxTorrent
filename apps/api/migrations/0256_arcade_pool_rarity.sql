-- 奖池档位的展示层：稀有度 rarity + 图标 image_url（2026-09-30，刮刮乐改版）
--
-- 动机：刮刮乐的「好不好看」几乎全在**揭晓那一刻** —— 稀有度决定配色/光效，
-- image_url 让魔力档也能挂一张图。两者都是**展示元数据**，不参与 EV 计算
-- （价值口径仍是 mult_permille / anchor），所以放在 arcade_pool_entries 上不会
-- 让「闸门看的数」和「前台画的东西」变成两份 —— 它们本就无关。
--
-- 与站点既有口径一致：所有池一起按档位价值分五档（写一次），此后站长可单独调。

BEGIN;

ALTER TABLE arcade_pool_entries
    ADD COLUMN IF NOT EXISTS rarity    smallint NOT NULL DEFAULT 1;
ALTER TABLE arcade_pool_entries
    ADD COLUMN IF NOT EXISTS image_url text     NOT NULL DEFAULT '';

ALTER TABLE arcade_pool_entries
    DROP CONSTRAINT IF EXISTS arcade_pool_entries_rarity_range;
ALTER TABLE arcade_pool_entries
    ADD CONSTRAINT arcade_pool_entries_rarity_range
        CHECK (rarity BETWEEN 1 AND 5);

-- 按档位价值分五档：0 →1（未中奖）、<1x →2、1x →3、<5x →4、≥5x →5（大奖）
-- 物品位给 4（稀有），站长可在后台单独调。
UPDATE arcade_pool_entries SET rarity = CASE
    WHEN kind = 'item'          THEN 4
    WHEN mult_permille = 0      THEN 1
    WHEN mult_permille < 1000   THEN 2
    WHEN mult_permille = 1000   THEN 3
    WHEN mult_permille < 5000   THEN 4
    ELSE 5
END;

DO $$
DECLARE
    bad bigint;
    n   bigint;
BEGIN
    SELECT count(*) INTO bad FROM arcade_pool_entries
     WHERE rarity < 1 OR rarity > 5;
    IF bad > 0 THEN
        RAISE EXCEPTION '% 行 rarity 越界', bad;
    END IF;
    SELECT count(*) INTO n FROM arcade_pool_entries WHERE rarity >= 4;
    RAISE NOTICE '奖池展示层就绪：高档位 % 行', n;
END $$;

COMMIT;
