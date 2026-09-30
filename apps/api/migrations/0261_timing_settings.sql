-- 钓鱼 / 宠物 时序参数进设置面板（2026-09-30）
--
-- 这两个玩法的时序此前只有代码缺省值（eco_i64 的 default），站长改不动。
-- 本迁移把 5 个键登记成**真设置键**（site_settings）+ 元数据（settings_meta），
-- 后台「娱乐模块」分组里即可调；缺省值与代码 default 一致，存量站零感知。
--
-- min/max 与代码里的 clamp 一致：面板拦住的，也正是代码会钳的范围 ——
-- 不让「填了却没生效」这种事发生。

BEGIN;

INSERT INTO site_settings (name, value, descr, grp) VALUES
    ('fishing_bite_min_ms', '1500', '钓鱼最短咬钩时间（毫秒）', 'module_fun'),
    ('fishing_bite_max_ms', '4500', '钓鱼最长咬钩时间（毫秒）', 'module_fun'),
    ('fishing_window_ms',   '1500', '钓鱼起竿窗口（毫秒）',   'module_fun'),
    ('pet_feed_cost',       '100',  '宠物投喂消耗（魔力）',   'module_fun'),
    ('pet_digest_per_hour', '300',  '宠物消化速度（能量/小时）', 'module_fun')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, unit, min, max, group_key, card_order)
VALUES
    ('fishing_bite_min_ms', 'number', '钓鱼最短咬钩', 'Fishing min bite',
     '毫秒', 300, 15000, 'module_fun', 30),
    ('fishing_bite_max_ms', 'number', '钓鱼最长咬钩', 'Fishing max bite',
     '毫秒', 300, 20000, 'module_fun', 31),
    ('fishing_window_ms',   'number', '钓鱼起竿窗口', 'Fishing reel window',
     '毫秒', 400, 5000, 'module_fun', 32),
    ('pet_feed_cost',       'number', '宠物投喂消耗', 'Pet feed cost',
     '魔力', 1, 100000, 'module_fun', 33),
    ('pet_digest_per_hour', 'number', '宠物消化速度', 'Pet digest / hour',
     '能量/小时', 1, 100000, 'module_fun', 34)
ON CONFLICT (name) DO UPDATE
    SET type = EXCLUDED.type,
        label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
        unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max,
        group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM settings_meta
     WHERE name IN ('fishing_bite_min_ms', 'fishing_bite_max_ms',
                    'fishing_window_ms', 'pet_feed_cost',
                    'pet_digest_per_hour');
    IF n < 5 THEN
        RAISE EXCEPTION '时序参数只登记了 % / 5 个', n;
    END IF;
    RAISE NOTICE '钓鱼/宠物时序参数已进设置面板：% 个键', n;
END $$;

COMMIT;
