-- 修 0248 播种时的一处口径错：`weight` 这一列的尺子是千分（九宫格标准池合计 1000），
-- 而 0248 把刮刮乐的**百分比**数字直接搬了进去（合计 100）。
--
-- 后果不是账面难看，是玩家看到的概率小十倍：公示与面板都按 weight/Σweight 现算，
-- 而前台按千分比渲染（`weight_permille / 10`）—— 45% 会显示成 4.5%。
-- 概率本身没错（权重同比缩放不影响抽样），错的是把它当千分读的那一侧。
UPDATE arcade_pool_entries
   SET weight = weight * 10
 WHERE pool_key = 'scratch_default'
   AND (SELECT sum(weight) FROM arcade_pool_entries
         WHERE pool_key = 'scratch_default') = 100;

DO $$
DECLARE w bigint;
BEGIN
    SELECT sum(weight) INTO w FROM arcade_pool_entries
     WHERE pool_key = 'scratch_default' AND enabled;
    IF w IS DISTINCT FROM 1000 THEN
        RAISE EXCEPTION '刮刮乐权重合计 %，不是 1000：千分口径没对齐', w;
    END IF;
END $$;
