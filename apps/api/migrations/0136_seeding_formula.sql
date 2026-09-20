-- 0133 做种激励公式改造：体积因子 + 个人做种时长分档 + 稀有度加成（产品决策 2026-09-19）
--
-- 背景（分析见 `_doc/做种激励公式横向对比.md`）：与 NexusPHP / Gazelle / UNIT3D 对照后，
-- 我们原有公式缺三根支柱，其中第一条是可被利用的漏洞：
--   ① 无体积因子 → 「堆小种」套利：1000 颗 1MB 种子（seeders=1）Σ≈250 → 时魔 ~205/h（≈封顶），
--      而保 6 颗 100GiB 濒危老种只有 89/h；NP 同场景仅 0.26/h、Gazelle ≈0（两家都有线性体积项）。
--   ② 个人做种时长是「衰减」乘数 1/(1+h/2160)（挂两年只剩 11%），而 Gazelle 对数递增、
--      UNIT3D 有 4 档加分 —— 三家都奖励长期保种，方向相反。
--   ③ 稀有度只有惩罚（seeders^-0.35，独苗=1），NP 是加成（独苗 ×2.414）。
--
-- 新公式（全部参数可配，缺省见下）：
--   每颗种子 s_i = ( 种子档位分 + 个人时长档位分 )
--                × min(1, log10(1+size_GB)/log10(1+vol_base))      -- 体积对数饱和
--                × (1 + rarity_k × seeders^-rarity_exp)            -- 稀有度加成
--                × scale                                           -- 总标定系数
--   时魔 = ( base + floor( (2/π)·cap·atan(Σ × curve_k) ) ) × (donor ? donor_mult : 1)
--
-- 标定（合成画像，脚本 `_calib2.py` 的结论，取 scale=0.4）：
--   画像                旧时魔  新时魔
--   轻度 1×5GB           11      12
--   中度 20×5GB          86      86   ← 持平（基准用户不受影响）
--   重度 60×2~30GB      133     141   ← +6%
--   保种党 10老+5濒危    70     134   ← +91%（正是要鼓励的行为）
--   堆小种 1000×1MB     205      10   ← -95%（堵住套利）
--
-- 幂等：ON CONFLICT，可重复执行。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('seeding_cap_hourly', '150', '做种时魔加成部分的渐近上限（arctan 封顶）', 'economy'),
('seeding_curve_k', '0.12', '做种时魔 atan 曲线系数（越大越快饱和）', 'economy'),
('seeding_vol_log_base_gb', '50', '体积因子对数基准（GB）：达到该体积记满分 1.0', 'economy'),
('seeding_rarity_k', '0.6', '稀有度加成系数：独苗得 (1+k) 倍，人多趋近 1', 'economy'),
('seeding_rarity_exp', '0.35', '稀有度衰减指数（seeders^-exp，Gazelle 口径）', 'economy'),
('seeding_bonus_scale', '0.4', '做种收益总标定系数（改公式后用它对账，见迁移注释）', 'economy'),
('seeding_donor_mult', '2', '捐赠者做种收益倍数（含底薪）', 'economy')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max, step, hint, group_key, card_order) VALUES
('seeding_cap_hourly', 'number', '做种封顶', 'Seeding cap', '魔力/小时', 10, 1000, 10,
 'arctan 渐近值：加成部分最多拿这么多（底薪另加）', 'economy', 40),
('seeding_curve_k', 'number', '做种曲线系数', 'Seeding curve k', '', 0.01, 1, 0.01,
 '越大越早饱和（Σ=10 时已吃掉大半收益）', 'economy', 41),
('seeding_vol_log_base_gb', 'number', '体积基准', 'Volume base', 'GB', 1, 1000, 1,
 '达到该体积记满分 1.0；小体积按 log 递减（1GB≈0.18、5GB≈0.46）', 'economy', 42),
('seeding_rarity_k', 'number', '稀有度加成', 'Rarity bonus', '', 0, 2, 0.05,
 '独苗得 (1+k) 倍收益；人多趋近 1（不惩罚热门）', 'economy', 43),
('seeding_rarity_exp', 'number', '稀有度指数', 'Rarity exponent', '', 0.1, 1, 0.05,
 'seeders^-exp 的指数，越大惩罚热门越重（Gazelle 用 0.35）', 'economy', 44),
('seeding_bonus_scale', 'number', '做种总标定', 'Seeding scale', '', 0.1, 2, 0.05,
 '整体倍率：调公式后用它把总量拉回原水平（缺省 0.4 已按画像标定）', 'economy', 45),
('seeding_donor_mult', 'number', '捐赠者倍数', 'Donor multiplier', 'x', 1, 5, 1,
 '捐赠者整笔（含底薪）翻倍', 'economy', 46)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en, unit = EXCLUDED.unit,
      min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
      hint = EXCLUDED.hint, group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- ============ 公式函数（单一真相源：worker 结算与 API 预估共用） ============

-- 参数行：从 site_settings 读一次（worker 用 CROSS JOIN LATERAL 复用同一行）
CREATE OR REPLACE FUNCTION seeding_params()
RETURNS TABLE (
    base          bigint,
    cap           double precision,
    curve_k       double precision,
    vol_base      double precision,
    rarity_k      double precision,
    rarity_exp    double precision,
    scale         double precision,
    donor_mult    bigint
) LANGUAGE sql STABLE AS $$
    SELECT
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_base_hourly')::bigint, 10),
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_cap_hourly')::double precision, 150),
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_curve_k')::double precision, 0.12),
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_vol_log_base_gb')::double precision, 50),
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_rarity_k')::double precision, 0.6),
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_rarity_exp')::double precision, 0.35),
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_bonus_scale')::double precision, 0.4),
        COALESCE((SELECT value FROM site_settings WHERE name = 'seeding_donor_mult')::bigint, 2)
$$;

-- 单颗种子的加成系数（**纯函数**：参数全部显式传入，便于直接做性质断言）
-- 注意：`p_age_days` 是种子发布至今（种龄）；`p_personal_hours` 是我挂它的累计小时。
CREATE OR REPLACE FUNCTION seeding_torrent_bonus(
    p_size            bigint,
    p_seeders         int,
    p_age_days        double precision,
    p_completed       int,
    p_personal_hours  double precision,
    p_vol_base        double precision,
    p_rarity_k        double precision,
    p_rarity_exp      double precision,
    p_scale           double precision
) RETURNS double precision LANGUAGE sql IMMUTABLE AS $$
    SELECT (
        -- 种子维度档位分（第一命中优先）
        (CASE
            WHEN p_seeders <= 1 AND p_completed >= 3 THEN 2.0
            WHEN p_age_days > 365 THEN 1.5
            WHEN p_age_days > 180 THEN 1.0
            WHEN p_size >= 107374182400 THEN 0.75
            WHEN p_size >= 26843545600 THEN 0.5
            ELSE 0.25
         END)
        -- 个人做种时长档位分（替代旧的 1/(1+h/2160) 衰减；三家同类项都是奖励长期保种）
        + (CASE
            WHEN p_personal_hours >= 8760 THEN 2.0
            WHEN p_personal_hours >= 4320 THEN 1.0
            WHEN p_personal_hours >= 2160 THEN 0.75
            WHEN p_personal_hours >= 720  THEN 0.5
            ELSE 0
         END)
    )
    -- 体积对数饱和：1MB≈0.0002、1GB≈0.18、5GB≈0.46、30GB≈0.87、≥vol_base 记 1.0
    * LEAST(1.0, ln(1 + GREATEST(p_size, 0) / 1073741824.0) / GREATEST(ln(1 + p_vol_base), 0.01))
    -- 稀有度加成：独苗 (1+k)，人多趋近 1（不惩罚热门）
    * (1 + p_rarity_k * power(GREATEST(p_seeders, 1), -p_rarity_exp))
    * p_scale
$$;

-- Σ加成 → 每小时魔力（底薪不参与压缩曲线；donor 整笔翻倍）
CREATE OR REPLACE FUNCTION seeding_hourly(
    p_sum_bonus   double precision,
    p_base        bigint,
    p_cap         double precision,
    p_curve_k     double precision,
    p_donor_mult  bigint,
    p_is_donor    boolean
) RETURNS bigint LANGUAGE sql IMMUTABLE AS $$
    SELECT (p_base + floor((2 / pi()) * p_cap * atan(GREATEST(p_sum_bonus, 0) * p_curve_k))::bigint)
         * (CASE WHEN p_is_donor THEN p_donor_mult ELSE 1 END)
$$;
