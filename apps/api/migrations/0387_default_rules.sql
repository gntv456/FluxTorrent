-- 0387 默认配置种子（0380-0386 各域的「开箱可用」数据）
--
-- 到此为止 0380~0386 建了表、状态机、接线与读面，但**一条规则都没配**——
-- 功能全在、效果为零。这条补上开箱可用的默认档。
--
-- 【幂等写法说明】medals 表**没有** name 唯一约束（既有勋章允许重名，
-- 例如「改名勋章」存在多条同名行），所以 `ON CONFLICT DO NOTHING`
-- 对它无效——重跑会插入重复行。改用 `WHERE NOT EXISTS (按 name 判)`：
-- 重跑零副作用，且不覆盖站长已改过的行。

-- ① 阈值类成就：六档。
--    取「新手够得着但要花点力气」的值。刻意不给「人人秒达成」的入门档
--    ——成就的价值在于有追求感，一次性发完等于没发。
INSERT INTO medals
    (name, description, rarity, auto_mode, auto_rule, auto_target,
     auto_period, progress_visible, sort_order)
SELECT v.name, v.descr, v.rarity, 'threshold', v.rule_name, v.target,
       'total', true, v.ord
FROM (VALUES
    ('勤勉学徒', '累计做种 100 小时。站点最缺的就是长期做种的号。',
     'bronze', 'seed_hours', 100::bigint, 10),
    ('勤勉常客', '累计做种 500 小时。',
     'silver', 'seed_hours', 500::bigint, 20),
    ('保种基石', '累计做种 2000 小时。站点的种子能活下来靠你们。',
     'gold', 'seed_hours', 2000::bigint, 30),
    ('初次播种', '发布 1 个通过审核的种子。',
     'bronze', 'uploads', 1::bigint, 40),
    ('勤勉园丁', '发布 10 个通过审核的种子。',
     'silver', 'uploads', 10::bigint, 50),
    ('老账号', '注册满 365 天。',
     'bronze', 'registered_days', 365::bigint, 60)
) AS v(name, descr, rarity, rule_name, target, ord)
WHERE NOT EXISTS (
    SELECT 1 FROM medals m
    WHERE m.name = v.name AND m.auto_mode IS NOT NULL
);

-- ② 事件类成就：**刻意不预置**。
--    它们需要业务侧调 record_event 才有输入，而站内目前还没接。
--    预置一条永远不动的进度条比不预置更糟——用户看到「0 / 100」会以为
--    自己没努力。接线时（record_event 有调用方了）再插这些行。

-- ③ H&R 零减免基线档（0384 已有 default/reduced/strict）。
--    这条的意义是**显式的「不减免」**：站长没主动配就是原样要求，
--    不做隐式放宽。未映射的等级本来就按 100% 走（见 domain::hr::tier_of），
--    这里只是让「不减免」在后台也可见可查。
INSERT INTO hr_tiers (tier, name, seed_hours_pct, days_pct, sort_order)
VALUES ('no_relief', '零减免档', 100, 100, 99)
ON CONFLICT (tier) DO NOTHING;

-- ④ H&R 减免地板（0384 已带，此处显式重述以免将来迁移顺序变化）
INSERT INTO site_settings (name, value) VALUES
    ('hr_relief_floor_hours', '1')
ON CONFLICT (name) DO NOTHING;

-- ⑤ 术语中台：既有 site_terms 已在 0386 迁移里登记成 category='site'，
--    这里**不动**它——站长的既有词表不该被打上别的标签。
