-- 猜大小「纯魔力 + 道具加权」+ 游戏道具（2026-09-30）
--
-- 两件事：
--  1) 猜大小**机制决定只输赢魔力**：清掉 bigsmall 池里的物品位（转成等值魔力位，
--     按 anchor/ticket 折算千分倍率，保持该档价值不缩水）；此后写侧与读侧都拒绝
--     bigsmall 出现 item 档（casino/pool 与 arcade_admin_write）。
--  2) 新增「游戏道具」这一 use_kind：道具不是奖品，而是在对局中**挂载后只改魔力
--     输赢幅度**（倍率 / 护盾）。它不改变「只输赢魔力」这条口径 —— 道具永不出物品。

BEGIN;

-- 1) 猜大小池：物品位 → 等值魔力位
--    注意：`payout` 列已被 0248 重命名为 `mult_permille`，这里只写后者。
UPDATE arcade_pool_entries e
   SET kind          = 'magic',
       item_key      = NULL,
       mult_permille = GREATEST(
           0,
           COALESCE(round(i.anchor * 1000.0 / NULLIF(p.ticket, 0)), 0)::bigint
       )
  FROM arcade_pools p, arcade_items i
 WHERE e.pool_key = p.key AND p.game = 'bigsmall'
   AND e.kind = 'item' AND i.key = e.item_key;

-- ── 2) 游戏道具：use_kind 增 'game'，物品目录加 game_effect ──────────────
ALTER TABLE arcade_items ADD COLUMN IF NOT EXISTS game_effect jsonb;

ALTER TABLE arcade_items DROP CONSTRAINT IF EXISTS arcade_items_use_kind_known;
ALTER TABLE arcade_items ADD CONSTRAINT arcade_items_use_kind_known
    CHECK (use_kind IN ('collect', 'spark', 'sku', 'game'));

-- game 类道具必须带得出 game_effect 的形状：没有它，挂载时无从知道改什么
ALTER TABLE arcade_items DROP CONSTRAINT IF EXISTS arcade_items_game_effect_shape;
ALTER TABLE arcade_items ADD CONSTRAINT arcade_items_game_effect_shape
    CHECK (
        use_kind <> 'game'
        OR (
            game_effect IS NOT NULL
            AND game_effect->>'game' IS NOT NULL
            AND game_effect->>'effect' IS NOT NULL
        )
    );

-- 消耗账：同时允许 game 类（并在 game 列里记是哪一款玩法用掉的，便于对账）
ALTER TABLE arcade_item_uses ADD COLUMN IF NOT EXISTS game text;

ALTER TABLE arcade_item_uses DROP CONSTRAINT IF EXISTS arcade_item_uses_kind_known;
ALTER TABLE arcade_item_uses ADD CONSTRAINT arcade_item_uses_kind_known
    CHECK (use_kind IN ('collect', 'spark', 'sku', 'game'));

-- ── 3) 播种两件猜大小道具（运营后续可增，见后台物品目录） ────────────────
-- effect/value 语义：mult 值=千分倍率（赢时派彩 × value/1000）；shield 值=千分返还
-- （输时返还 value/1000 注额）。两者互斥生效（mult 只在赢、shield 只在输）。
INSERT INTO arcade_items
    (key, name, kind, anchor, anchor_src, unlimited, stock, per_user,
     icon, enabled, sort, use_kind, use_ref, game_effect)
VALUES
    ('bs_mult2', '倍率券 · 双倍', 'voucher', 0, 'n/a', true, 0, 5,
     '2️⃣', true, 200, 'game', '',
     '{"game":"bigsmall","effect":"mult","value":2000}'::jsonb),
    ('bs_shield50', '护盾 · 返还半注', 'voucher', 0, 'n/a', true, 0, 5,
     '🛡', true, 210, 'game', '',
     '{"game":"bigsmall","effect":"shield","value":500}'::jsonb)
ON CONFLICT (key) DO NOTHING;

-- ── 4) 落库断言 ──────────────────────────────────────────────────────────
DO $$
DECLARE
    n     bigint;
    bad   bigint;
BEGIN
    -- 猜大小池里不允许再有物品位（清理没生效就炸，别留给运行时）
    SELECT count(*) INTO n
      FROM arcade_pool_entries e
      JOIN arcade_pools p ON p.key = e.pool_key
     WHERE p.game = 'bigsmall' AND e.kind = 'item';
    IF n > 0 THEN
        RAISE EXCEPTION '猜大小池里仍有 % 个物品位：纯魔力化没清干净', n;
    END IF;

    -- 道具的 game_effect 必须是可执行形状（effect ∈ mult|shield，value>0）
    SELECT count(*) INTO bad FROM arcade_items
     WHERE use_kind = 'game'
       AND (game_effect->>'effect' NOT IN ('mult', 'shield')
            OR COALESCE((game_effect->>'value')::bigint, 0) <= 0);
    IF bad > 0 THEN
        RAISE EXCEPTION '% 件道具的 game_effect 形状非法', bad;
    END IF;

    RAISE NOTICE '猜大小纯魔力化 + 游戏道具就绪';
END $$;

COMMIT;
