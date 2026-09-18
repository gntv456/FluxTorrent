-- 0127 娱乐玩法「通用化 + 回收口径」统一（产品决策 2026-09-19）
--
-- 背景：FluxTorrent 是通用 PT 建站系统（可建成教育/影音/音乐/综合等任何站型），
-- 娱乐玩法不得携带任何特定站型的特色（旧数据的作物名是教育站色彩），
-- 且全部玩法必须以**回收魔力**为目的 —— 期望回报一律 < 1。
--
-- 本次修正三件事：
--   1) 农场作物通用化命名（幻想系，无站型色彩，站长可自行改名）+ 产量重标定到回收口径；
--   2) 猜大小赔率 2x → 1.9x（2x 时 EV 恰为 1.0，既不回收又可双向零风险对冲）；
--   3) 清理游戏域里残留的站型特色文案（模块描述 / 设置项标签 / 趣味盒问题）。
--
-- 幂等：全部语句可重复执行；已被站长改过的名称/文案不会被覆盖（用 WHERE 守卫），
-- 但**产量与赔率无条件修正**（经济安全优先）。

-- ============ 1. 农场作物：通用命名 + 回收标定 ============
-- 回收口径：base_yield = seed_price × 0.75，含 20% 双倍收获后期望回报 = 0.90 < 1。
-- 五档一致（不改 grow_hours / seed_price 的档位设计，只调产量）。
INSERT INTO farm_crops (id, name, seed_price, base_yield, grow_hours) VALUES
    (1, '四叶草',  100,   75,  4),
    (2, '星尘豆',  300,  225, 12),
    (3, '云端瓜',  800,  600, 24),
    (4, '月华参', 2000, 1500, 48),
    (5, '日冕稻', 5000, 3750, 96)
ON CONFLICT (id) DO UPDATE SET base_yield = EXCLUDED.base_yield;

-- 名称仅在仍是旧的教育站命名时替换（尊重站长已自定义的名字）
UPDATE farm_crops SET name = '四叶草' WHERE id = 1 AND name IN ('知识麦');
UPDATE farm_crops SET name = '星尘豆' WHERE id = 2 AND name IN ('智慧豆');
UPDATE farm_crops SET name = '云端瓜' WHERE id = 3 AND name IN ('学问瓜');
UPDATE farm_crops SET name = '月华参' WHERE id = 4 AND name IN ('博士参');
UPDATE farm_crops SET name = '日冕稻' WHERE id = 5 AND name IN ('状元稻');

-- ============ 2. 猜大小赔率：2x → 1.9x ============
-- 赢面 49% / 平局 2% / 输面 49%（1-49 小、52-100 大、50/51 平局返本）；
-- EV = 0.49 × 赔率 + 0.02 × 1。2.0 → 1.00（中性，且押两边可零风险对冲）；
-- 1.9 → 0.951（回收 4.9%）。代码侧同步加了单测锁（games::tests::bigsmall_expected_value_below_one）。
UPDATE site_settings SET value = '1.9'
 WHERE name = 'games_bigsmall_win_mult' AND value IN ('2', '2.0');

INSERT INTO site_settings (name, value) VALUES ('games_bigsmall_win_mult', '1.9')
ON CONFLICT (name) DO NOTHING;

-- 后台设定项：改为支持小数的步进与更合理的上下界（1.0 = 不赚，2.0 = 中性红线）
UPDATE settings_meta
   SET unit = 'x', min = 1.0, max = 2.0, step = 0.05,
       hint = '必须小于 2.0：2.0 时期望回报恰为 1（不回收）且可双向对冲无风险'
 WHERE name = 'games_bigsmall_win_mult';

-- 顺手补齐刮刮乐 2x 档的设定项标签（0109 已建键，此前未被代码消费）
UPDATE settings_meta SET hint = '与空奖/保底/1x 合计不得超过 100%，余数归 10x 档'
 WHERE name IN ('games_scratch_empty_pct', 'games_scratch_half_pct',
                'games_scratch_one_pct', 'games_scratch_two_pct');

-- ============ 3. 去站型特色文案（游戏域） ============
-- 模块描述
UPDATE modules SET descr = replace(replace(descr, '好学农场', '农场'), '种下知识', '种下种子')
 WHERE descr LIKE '%好学农场%' OR descr LIKE '%种下知识%';
UPDATE modules SET descr = '刮刮乐 / 猜大小 / 九宫格 / 趣味投票（合规敏感：机会类玩法）'
 WHERE key = 'games' AND descr LIKE '%好学%';

-- 设置项标签（后台站点设定页可见）
UPDATE settings_meta
   SET label_zh = replace(label_zh, '好学农场', '农场')
 WHERE label_zh LIKE '%好学农场%';

-- 趣味盒示例问题（教育站口径 → 通用）
UPDATE fun_polls SET question = replace(question, '深夜下载学习资料时', '深夜下载资源时')
 WHERE question LIKE '%学习资料%';
