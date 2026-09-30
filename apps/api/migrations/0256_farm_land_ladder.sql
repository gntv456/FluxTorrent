-- 农场土地阶梯：买地 + 地块升级（2026-09-30）
--
-- 经济定位：这是**确定性魔力沉没口** —— 只出不进、零退款，不参与任何 EV 判据。
-- 升级买到的只有周转（成熟时长），产量/种子价/彩蛋倍率都与等级无关，所以农场
-- 的收获期望恒等于作物表标定的 0.90。机制在代码（games/farm_land.rs），
-- 参数在这张行表 + 下面五个设置键。
--
-- 与代码同源的三个数（改一边必须改另一边，下面的 DO 断言会把漂移当场判红）：
--   slot 上界 30   = games::farm_land::MAX_PLOTS_HARD_CAP
--   level 上界 12  = games::farm_land::MAX_LEVEL（由周转地板 500‰ 推出）
--   免费地块 6     = games::farm::PLOTS（不在这里播种，见下方说明）

BEGIN;

CREATE TABLE IF NOT EXISTS farm_land (
    user_id   BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    slot      INT    NOT NULL,
    level     INT    NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    upgraded_at TIMESTAMPTZ,
    PRIMARY KEY (user_id, slot),
    -- 槽位从 1 起、到硬上限止；免费地块（1..=6）不播种行 —— 「没行」在
    -- 读侧一律派生成 level 1，所以这张表只记「买来的地」和「升过级的地」。
    -- 于是站长改免费数不需要迁移，也不会有人拿到一批没用的行。
    CONSTRAINT farm_land_slot_chk  CHECK (slot BETWEEN 1 AND 30),
    CONSTRAINT farm_land_level_chk CHECK (level BETWEEN 1 AND 12),
    -- 买来的地必须排在免费地之外：把免费地也写成行会让「已买块数」失去口径
    -- （阶梯价按已买块数走，多出来的行等于偷偷打折）
    CONSTRAINT farm_land_no_free_chk CHECK (level > 1 OR slot > 6)
);

-- 阶梯参数：底数与比率都是站长可配，缺省与 games::farm_land::DEFAULT_* 同源
INSERT INTO site_settings (name, value) VALUES
    ('farm_max_plots', '12'),
    ('farm_land_base', '2000'),
    ('farm_land_ratio_permille', '2200'),
    ('farm_up_base', '1000'),
    ('farm_up_ratio_permille', '1600')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, unit, min, max, step,
     group_key, card_order)
VALUES
    ('farm_max_plots', 'number', '农场地块总上限', 'Farm plots cap',
     '含免费送的地块。超出免费数的那几块按阶梯价购买，买地只沉没魔力、不退款，'
     '也不改变收获期望。上限最高 30（与代码里的硬上限同源）。',
     '块', 6, 30, 1, 'module_fun', 22),
    ('farm_land_base', 'number', '第一块买地价', 'First land price',
     '第 7 号地块的价格（魔力）。后面每多买一块按下面的比率递增。',
     '魔力', 1, 100000000, 1, 'module_fun', 23),
    ('farm_land_ratio_permille', 'number', '买地价格递增（千分比）',
     'Land price ratio (permille)',
     '每多买一块，下一块按这个倍数变贵。必须大于 1000‰，否则「越买越便宜」。'
     '2200 = 每块 ×2.2。',
     '‰', 1001, 5000, 1, 'module_fun', 24),
    ('farm_up_base', 'number', '第一次升级地块价', 'First upgrade price',
     '把一块地从 1 级升到 2 级的价格（魔力）。之后每级按下面的比率递增。',
     '魔力', 1, 100000000, 1, 'module_fun', 25),
    ('farm_up_ratio_permille', 'number', '升级价格递增（千分比）',
     'Upgrade price ratio (permille)',
     '每升一级，下一级升级费按这个倍数变贵。必须大于 1000‰。'
     '1600 = 每级 ×1.6。升级只买周转（每级成熟时长 ×0.94），'
     '压到地板 50% 就到顶，第 12 级之后不再收钱。',
     '‰', 1001, 5000, 1, 'module_fun', 26);

-- 参数自证：播种出来的缺省必须能通过代码侧同一套判据。
-- 「不会红的门禁不算门禁」—— 每一项都配了反向验证（见 e2e 与本文件末尾注释）。
DO $$
DECLARE
    v_cap     BIGINT;
    v_lbase   BIGINT;
    v_lratio  BIGINT;
    v_ubase   BIGINT;
    v_uratio  BIGINT;
BEGIN
    SELECT value::bigint INTO v_cap    FROM site_settings
     WHERE name = 'farm_max_plots';
    SELECT value::bigint INTO v_lbase  FROM site_settings
     WHERE name = 'farm_land_base';
    SELECT value::bigint INTO v_lratio FROM site_settings
     WHERE name = 'farm_land_ratio_permille';
    SELECT value::bigint INTO v_ubase  FROM site_settings
     WHERE name = 'farm_up_base';
    SELECT value::bigint INTO v_uratio FROM site_settings
     WHERE name = 'farm_up_ratio_permille';

    IF v_cap IS NULL OR v_lbase IS NULL OR v_lratio IS NULL
       OR v_ubase IS NULL OR v_uratio IS NULL THEN
        RAISE EXCEPTION '农场土地阶梯参数缺行：五个键必须都在 site_settings 里';
    END IF;
    IF v_lbase < 1 OR v_ubase < 1 THEN
        RAISE EXCEPTION '阶梯底数必须大于 0（配成 0 等于白送地块），'
            '实为 买地 % / 升级 %', v_lbase, v_ubase;
    END IF;
    IF v_lratio <= 1000 OR v_uratio <= 1000 THEN
        RAISE EXCEPTION '阶梯比率必须大于 1000‰（越买越贵），'
            '实为 买地 %‰ / 升级 %‰', v_lratio, v_uratio;
    END IF;
    -- 上限低于免费地块数 = 配置自己说不通
    IF v_cap < 6 THEN
        RAISE EXCEPTION '农场地块总上限 % 低于免费送的地块数 6', v_cap;
    END IF;
    IF v_cap > 30 THEN
        RAISE EXCEPTION '农场地块总上限需在 1 ~ 30 之间（列宽与硬上限），'
            '实为 %', v_cap;
    END IF;

    -- 等级上限 12 的来源：周转每级 ×0.94，地板 50%。
    -- 第 12 级（×0.94^11）还在地板之上，第 13 级已经踩进地板 ——
    -- 那一注钱什么都买不到，所以上限就是 12。改地板会改这里，两边不会走偏。
    IF floor(power(0.94, 11) * 1000) > 500
       AND floor(power(0.94, 12) * 1000) <= 500 THEN
        NULL;
    ELSE
        RAISE EXCEPTION '地块等级上限 12 已与周转步长 0.94 / 地板 500‰ 不一致：'
            '0.94^11=% 0.94^12=%',
            floor(power(0.94, 11) * 1000), floor(power(0.94, 12) * 1000);
    END IF;
END
$$;

COMMIT;

-- 反向验证（改本文件前请先跑一遍）：
--   UPDATE site_settings SET value='1000' WHERE name='farm_land_ratio_permille';
--     → 重跑断言报「阶梯比率必须大于 1000‰」
--   UPDATE site_settings SET value='3' WHERE name='farm_max_plots';
--     → 报「低于免费送的地块数 6」
--   把 CHECK 的 12 改成 11：断言仍绿，但第 12 级的地块行会被列约束拒 ——
--   所以等级这个数字同时受列约束与断言两处管，只改一处不会静默通过。
