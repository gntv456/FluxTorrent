-- 物品用途（产品口径：所有奖品都是站内虚拟物品，抽到必须「在站内能用」）。
-- 0244 只把「发得出去」建起来，发出去的东西没有下一步，等于奖品是张欠条。
-- 本文件加消耗侧：一件物品从背包里用掉，走三条用途之一。
--
--   collect —— 仅收藏/图鉴计数，无消耗、无负债（默认值，存量行为不变）；
--   spark   —— 按权威折算价兑现成魔力：这份负债在奖池 EV 里已按 anchor 计入，
--              兑现只是「实现时点」不同，不新增发行；
--   sku     —— 复用商店生效链 apply_item_effect（装扮/券/上传量/VIP 都在那边），
--              这里只记「绑哪件 SKU」，绝不在娱乐屋另写一份效果实现（第二份清单必然漂移）。
ALTER TABLE arcade_items ADD COLUMN IF NOT EXISTS use_kind text NOT NULL DEFAULT 'collect';
ALTER TABLE arcade_items ADD COLUMN IF NOT EXISTS use_ref  text NOT NULL DEFAULT '';

ALTER TABLE arcade_items DROP CONSTRAINT IF EXISTS arcade_items_use_kind_known;
ALTER TABLE arcade_items ADD CONSTRAINT arcade_items_use_kind_known
    CHECK (use_kind IN ('collect', 'spark', 'sku'));

-- 消耗账：与发放账同构，「持有 = 发放 − 消耗」由两张账反推，
-- 不在 users 或别处再存一份持有数（那才是真正会漂移的第三份清单）。
CREATE TABLE IF NOT EXISTS arcade_item_uses (
    id        bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    item_key  text NOT NULL REFERENCES arcade_items (key) ON DELETE CASCADE,
    user_id   bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    qty       integer NOT NULL DEFAULT 1,
    use_kind  text NOT NULL,
    idem      text,
    used_at   timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT arcade_item_uses_idem_unique UNIQUE (idem),
    CONSTRAINT arcade_item_uses_qty_positive CHECK (qty > 0),
    CONSTRAINT arcade_item_uses_kind_known
        CHECK (use_kind IN ('collect', 'spark', 'sku'))
);

CREATE INDEX IF NOT EXISTS idx_arcade_uses_user_item
    ON arcade_item_uses (user_id, item_key);

-- 我的背包（按物品聚合的持有数）。玩法侧发得出、玩家侧用得掉，
-- 中间这一份「还剩几件」只能有一个算法。
CREATE OR REPLACE VIEW arcade_item_held AS
    SELECT gr.user_id,
           gr.item_key,
           gr.granted,
           COALESCE(us.used, 0)::bigint          AS used,
           (gr.granted - COALESCE(us.used, 0))::bigint AS held
      FROM (SELECT user_id, item_key, SUM(qty)::bigint AS granted
              FROM arcade_item_grants
             GROUP BY user_id, item_key) gr
      LEFT JOIN (SELECT user_id, item_key, SUM(qty)::bigint AS used
              FROM arcade_item_uses
             GROUP BY user_id, item_key) us
        ON us.user_id = gr.user_id AND us.item_key = gr.item_key;

-- ── 把目录里能用起来的两件接上用途 ────────────────────────────────────────
-- 经济/凭证类按折算价兑现；卡框绑站内正在卖的头像框 SKU（按 SKU 售价重报 anchor，
-- 因为「能用」之后它的价值就是那件装扮，不再是零负债展示品）。
-- SKU 按 kind 查而不是写死 id：站型包不同，货架也不同。
UPDATE arcade_items SET use_kind = 'spark', updated_at = now()
 WHERE key IN ('pass3', 'makeup', 'bank500', 'ticket');

DO $$
DECLARE
    sku_id  bigint;
    sku_pr  bigint;
BEGIN
    SELECT id, price INTO sku_id, sku_pr
      FROM shop_items
     WHERE kind = 'avatar_frame' AND active
     ORDER BY price DESC, id
     LIMIT 1;
    IF found THEN
        UPDATE arcade_items
           SET use_kind = 'sku', use_ref = sku_id::text,
               anchor = sku_pr, anchor_src = 'shop', updated_at = now()
         WHERE key = 'frame';
        RAISE NOTICE '卡框奖品接上装扮生效链：SKU %（售价 %）', sku_id, sku_pr;
    ELSE
        RAISE NOTICE '站内没有在售头像框 SKU，frame 保持 collect（不硬绑）';
    END IF;
END $$;

-- ── 落库断言：用途本身不能是自相矛盾的配置 ────────────────────────────────
DO $$
DECLARE
    bad bigint;
BEGIN
    -- 绑 SKU 就必须真买得到：SKU 存在、在售，且 anchor 不得低于售价 ——
    -- 否则奖池按 anchor 计的负债，小于这张券实际兑出去的东西，是条印钞缝。
    SELECT count(*) INTO bad
      FROM arcade_items i
     WHERE i.use_kind = 'sku'
       AND (i.use_ref !~ '^[0-9]+$'
            OR NOT EXISTS (SELECT 1 FROM shop_items s
                            WHERE s.id::text = i.use_ref
                              AND s.active
                              AND i.anchor >= s.price));
    IF bad > 0 THEN
        RAISE EXCEPTION '% 个物品绑的 SKU 不存在/已下架/anchor 低于售价', bad;
    END IF;

    -- 可兑现物品必须报得出正折算价，anchor<=0 的「兑现」就是凭空印钞
    SELECT count(*) INTO bad FROM arcade_items WHERE use_kind = 'spark' AND anchor <= 0;
    IF bad > 0 THEN
        RAISE EXCEPTION '% 个 spark 物品 anchor<=0：兑现等于印钞', bad;
    END IF;
END $$;
