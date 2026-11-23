-- 0517 扭蛋机替换为「老虎机 + 水果机」两款（2026-11-21）。
--
-- 背景：原娱乐屋扭蛋（capsule）整体下线入口，同构池抽模型拆成两款：
--   · 老虎机 slot —— 经典三滚轮 777 街机造型；后端复用旧 capsule 池/ref_type，
--     历史流水、票根成就（capsule_plays）、图鉴与幂等回放不断档；
--   · 水果机 fruit —— 参考图水果押注面板（铃铛/苹果/西瓜/橙子/柠檬/星星/77/BAR），
--     全新池键 fruit，奖池/流水/图鉴独立。
--
-- 经济口径与既有池抽一致：票价 100、权重合计 1000、EV < 1（回收口纪律），
-- 落库用 DO 块断言，写错在迁移阶段就炸掉。

BEGIN;

-- ── 1) 奖池 ──────────────────────────────────────────────────────────────
-- 老虎机：旧 capsule_default 池改挂老虎机标签（档位与 EV 保留不动；站长若已
-- 自行调过档位，这里只更 label，不 DELETE entries）。
UPDATE arcade_pools
   SET label = '老虎机 · 标准池', updated_at = now()
 WHERE key = 'capsule_default';

-- 水果机：新建标准池。档位形状（权重 : 倍率千分比）：
-- 谢谢 620:0 · 柠檬 130:800 · 甜橙 90:1200 · 苹果 70:1800 ·
-- 西瓜 50:3000 · 金铃铛 25:5000 · 双星 12:10000 · 777 大奖 3:40000。
-- 权重合计 1000，EV = 0.853 < 1。
INSERT INTO arcade_pools (key, game, label, ticket, enabled, sort)
VALUES ('fruit_default', 'fruit', '水果机 · 标准池', 100, true, 41)
ON CONFLICT (key) DO NOTHING;

DELETE FROM arcade_pool_entries WHERE pool_key = 'fruit_default';

INSERT INTO arcade_pool_entries
    (pool_key, label, weight, kind, mult_permille, side, enabled, sort,
     rarity)
VALUES
    ('fruit_default', '谢谢参与', 620, 'magic',    0, 'any', true,  10, 1),
    ('fruit_default', '柠檬',     130, 'magic',  800, 'any', true,  20, 1),
    ('fruit_default', '甜橙',      90, 'magic', 1200, 'any', true,  30, 2),
    ('fruit_default', '红苹果',    70, 'magic', 1800, 'any', true,  40, 2),
    ('fruit_default', '西瓜',      50, 'magic', 3000, 'any', true,  50, 3),
    ('fruit_default', '金铃铛',     25, 'magic', 5000, 'any', true,  60, 3),
    ('fruit_default', '双星',       12, 'magic',10000, 'any', true,  70, 4),
    ('fruit_default', '777 大奖',    3, 'magic',40000, 'any', true,  80, 5);

DO $$
DECLARE
    p     text;
    w     bigint;
    ev    numeric;
BEGIN
    FOREACH p IN ARRAY ARRAY['capsule_default', 'fruit_default'] LOOP
        SELECT sum(weight) INTO w FROM arcade_pool_entries
         WHERE pool_key = p AND enabled;
        IF w IS DISTINCT FROM 1000 THEN
            RAISE EXCEPTION '% 权重合计 %，不是 1000', p, w;
        END IF;
        SELECT sum(weight * mult_permille)::numeric / 1000 / 1000 INTO ev
          FROM arcade_pool_entries WHERE pool_key = p AND enabled;
        IF ev >= 1 THEN
            RAISE EXCEPTION '% EV % >= 1：在增发，拒绝该配置落库', p, ev;
        END IF;
        RAISE NOTICE '% 行表化完成：权重 %，EV %', p, w, ev;
    END LOOP;
END $$;

-- ── 2) 大厅玩法清单：旧 capsule 入口撤下，换 slot / fruit 两行 ─────────────
DELETE FROM arcade_games WHERE key = 'capsule';

INSERT INTO arcade_games
    (key, title_key, icon, href, tone, grp, module_gate, badge, hot,
     enabled, sort)
VALUES
    ('slot',  'slot',  '🎰', '/games/slot',  'indigo', 'instant', '',
     'capsule', true,  true, 40),
    ('fruit', 'fruit', '🍒', '/games/fruit', 'coral',  'instant', '',
     'fruit',  false, true, 41)
ON CONFLICT (key) DO UPDATE SET
    title_key = EXCLUDED.title_key,
    icon = EXCLUDED.icon,
    href = EXCLUDED.href,
    tone = EXCLUDED.tone,
    grp = EXCLUDED.grp,
    module_gate = EXCLUDED.module_gate,
    badge = EXCLUDED.badge,
    hot = EXCLUDED.hot,
    enabled = EXCLUDED.enabled,
    sort = EXCLUDED.sort,
    updated_at = now();

-- ── 3) 等级门槛：水果机新键；老虎机沿用旧 capsule 门槛键 ──────────────────
INSERT INTO site_settings (name, value, descr, grp) VALUES
    ('games_minclass_fruit', '0', '水果机最低等级', 'games')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint,
                           group_key, card_order) VALUES
    ('games_minclass_fruit', 'classlevel', '水果机等级门槛',
     'Fruit machine min class',
     '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
     '娱乐屋', 10)
ON CONFLICT (name) DO NOTHING;

COMMIT;
