-- 娱乐屋大厅「玩法清单」行表化（2026-09-30）
--
-- 此前大厅卡片写在前端 `lib/games-registry.ts`：加一款要改代码，站长也没法
-- 调序/下架。落成行表后，大厅读表渲染 —— 后台可排序、可隐藏、可换图标/配色。
--
-- 口径：本表只管**展示**（顺序/显隐/图标/色系/角标来源），不承载任何经济参数；
-- 玩法本身的能力（奖池/道具/池）仍在各自域里。前端仍保留一份注册表作**兜底**：
-- 表为空（旧库未跑迁移）时大厅照常出 9 张卡，不会白屏。

BEGIN;

CREATE TABLE IF NOT EXISTS arcade_games (
    key         text PRIMARY KEY,
    title_key   text NOT NULL DEFAULT '',   -- 预留：字典键覆盖（空=用 key）
    icon        text NOT NULL,
    href        text NOT NULL,
    tone        text NOT NULL DEFAULT 'sky',
    grp         text NOT NULL DEFAULT 'session',   -- instant | session
    module_gate text NOT NULL DEFAULT '',   -- 站点模块开关名（空=games）
    badge       text NOT NULL DEFAULT '',   -- 角标数据来源（/games 概览字段）
    hot         boolean NOT NULL DEFAULT false,
    enabled     boolean NOT NULL DEFAULT true,
    sort        integer NOT NULL DEFAULT 0,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT arcade_games_grp_known CHECK (grp IN ('instant', 'session'))
);

INSERT INTO arcade_games
    (key, title_key, icon, href, tone, grp, module_gate, badge, hot, enabled, sort)
VALUES
    ('bigsmall', 'bigsmall', '🎯', '/games/bigsmall', 'sky',    'instant', '',      'bigsmall', true,  true,  10),
    ('scratch',  'scratch',  '🎫', '/games/scratch',  'coral',  'instant', '',      'scratch',  true,  true,  20),
    ('jgg',      'jgg',      '🎰', '/games/jgg',      'violet',  'instant', '',      'jgg',      false, true,  30),
    ('capsule',  'capsule',  '🥚', '/games/capsule',  'sun',    'instant', '',      'capsule',  false, true,  40),
    ('wheel',    'wheel',    '🎡', '/games/wheel',    'sun',    'instant', '',      'wheel',    true,  true,  50),
    ('farm',     'farm',     '🌾', '/games/farm',     'mint',   'session', 'farm',  'farm',     false, true,  60),
    ('gacha',    'gacha',    '🃏', '/gacha',          'indigo', 'session', 'gacha', '',         false, true,  70),
    ('pet',      'pet',      '🐾', '/games/pet',      'teal',   'session', '',      '',         false, true,  80),
    ('fishing',  'fishing',  '🎣', '/games/fishing',  'sky',    'session', '',      'fishing',  false, true,  90)
ON CONFLICT (key) DO NOTHING;

DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM arcade_games WHERE enabled;
    IF n < 9 THEN
        RAISE EXCEPTION '大厅玩法清单只播种了 % 条（应 >= 9）', n;
    END IF;
    RAISE NOTICE '大厅玩法清单就绪：% 条启用', n;
END $$;

COMMIT;
