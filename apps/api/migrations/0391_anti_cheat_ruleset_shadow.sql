-- 0391_anti_cheat_ruleset_shadow.sql
-- 反作弊规则集版本化 + 影子模式 + 回测（评估报告 §1.2）
--
-- 现状（0368 risk_score.rs）：权重/阈值全部 site_settings 化，但
--  * `RULESET_VER` 是**硬编码常量**——改公式或调权后，历史分数行的
--    版本号还是旧的，面板上「这批分数是哪版权重算的」无从判断；
--  * 调权**立即生效**于自动开案：改一个权重，下一轮 risk_score 就可能
--    给一批人开案。没有「先影子跑、看看会多判多少人」这一步，
--    改参数就是闭眼按扳机——这正是「敢不敢开自动处置」缺的信任基础设施；
--  * 无回测：无法回答「改成这版权重，过去 90 天会多判/少判哪些人」。
--
-- 本迁移补三样：
--   ① cheat_rulesets：规则集版本本体（权重/阈值/公式的快照 + 生效区间）。
--      改参数即新增一版，旧版保留——历史分数行指向的版本永远查得到。
--   ② cheat_shadow_scores：影子模式下的**候选**分数，不触发开案。
--      与正式分数分开存，避免影子结果污染案件队列。
--   ③ cheat_backtest_runs / cheat_backtest_diffs：回测结论
--      （改这版权重会多判/少判谁、差多少档）。
--
-- 【不改变的部分】risk_score 的公式与自动开案判据一字未动。
-- 本条只是把「参数从散落的 site_settings」升级为「有版本、可影子、
-- 可回测的参数集」，并让分数行记下真实版本号。

-- ① 规则集版本
CREATE TABLE IF NOT EXISTS cheat_rulesets (
    version     INT         PRIMARY KEY,
    -- 整份参数快照（权重/阈值/半衰期/NORM…）。只存 JSONB，
    -- 因为参数的键会随公式演进而增删——用固定列会让每次调参都写迁移。
    params      JSONB       NOT NULL,
    note        TEXT        NOT NULL DEFAULT '',
    -- 是否已正式生效（false = 草稿/影子中）
    active      BOOLEAN     NOT NULL DEFAULT false,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    activated_at TIMESTAMPTZ
);

COMMENT ON TABLE cheat_rulesets IS
    '反作弊规则集版本（0391）。params 是权重/阈值/半衰期的全量快照：'
    '用 JSONB 而非固定列，因为键会随公式演进而增删，用列就得每次调参写迁移。'
    '分数行记 ruleset_ver → 任何时候都能查回「这批分数是哪版权重算的」。';

-- 同一时刻只允许一个 active 版本
CREATE UNIQUE INDEX IF NOT EXISTS ux_cheat_rulesets_active
    ON cheat_rulesets ((active)) WHERE active;

COMMENT ON INDEX ux_cheat_rulesets_active IS
    '同一时刻只允许一个 active 版本（部分唯一索引）。'
    '切换版本必须先关旧再开新，两条都开会让分数行版本号指向不明。';

-- ② 影子分数（不触发开案）
CREATE TABLE IF NOT EXISTS cheat_shadow_scores (
    user_id      BIGINT      PRIMARY KEY,
    ruleset_ver  INT         NOT NULL,
    -- 影子算出的分数（未生效）
    score        INT         NOT NULL,
    band         TEXT        NOT NULL,
    -- 与当前生效分数的差（正 = 影子更可疑）
    delta_vs_live INT        NOT NULL DEFAULT 0,
    -- 会不会因此开案（band 到 high/critical 且无进行中案件）
    would_open   BOOLEAN     NOT NULL DEFAULT false,
    breakdown    JSONB       NOT NULL DEFAULT '{}'::jsonb,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE cheat_shadow_scores IS
    '影子模式的候选分数（0391）。与正式分数**分开存**：'
    '影子结果若混进正式分数表，"调参会多判多少人"这个问题本身就无法回答了。'
    'would_open 直接回答「这版权重会不会给人开案」。';

CREATE INDEX IF NOT EXISTS idx_cheat_shadow_would_open
    ON cheat_shadow_scores (score DESC) WHERE would_open;

COMMENT ON INDEX idx_cheat_shadow_would_open IS
    '影子模式下会被开案的用户（部分索引）。改参数前先看这一撮有多大。';

-- ③ 回测
CREATE TABLE IF NOT EXISTS cheat_backtest_runs (
    id           BIGSERIAL   PRIMARY KEY,
    from_ver     INT         NOT NULL,
    to_ver       INT         NOT NULL,
    -- 回测窗口天数
    window_days  INT         NOT NULL DEFAULT 90,
    -- 结论摘要
    total_users  INT         NOT NULL DEFAULT 0,
    would_open   INT         NOT NULL DEFAULT 0,
    band_up      INT         NOT NULL DEFAULT 0,
    band_down    INT         NOT NULL DEFAULT 0,
    -- 逐人差异（只存会开案/会掉档的，全量差异无意义且占空间）
    diffs        JSONB       NOT NULL DEFAULT '[]'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE cheat_backtest_runs IS
    '反作弊规则集回测（0391）。回答「换成这版权重，过去 N 天会'
    '多判/少判谁」。diffs 只存会开案或会掉档的行——'
    '全量差异既无意义又占空间。';

CREATE INDEX IF NOT EXISTS idx_cheat_backtest_time
    ON cheat_backtest_runs (created_at DESC);

-- ④ 影子/回测开关
INSERT INTO site_settings (name, value) VALUES
    -- 1 = 影子模式：新参数只算分不开案；0 = 立即生效
    ('risk_shadow_mode', '1'),
    ('risk_shadow_max_ver', '0')
ON CONFLICT (name) DO NOTHING;

COMMENT ON TABLE site_settings IS
    '站点配置。risk_shadow_mode：1 = 影子模式（规则集版本只算分不开案，'
    '运营对比无回归后再关掉）；0 = 立即生效。'
    'risk_shadow_max_ver：影子模式下的最大影子版本（0 = 不限制）。';

-- ⑤ 种子：把当前生效参数固化成 v1（active）。
--    从此分数行记的 ruleset_ver 指向一张真实存在的版本，而不是硬编码常量。
INSERT INTO cheat_rulesets (version, params, note, active, activated_at)
SELECT 1,
       jsonb_object_agg(name, value),
       '0368 存量参数固化（0391 建表时的实际生效值）',
       true, now()
  FROM site_settings
 WHERE name LIKE 'risk\_%'
   AND name NOT IN ('risk_shadow_mode', 'risk_shadow_max_ver')
ON CONFLICT (version) DO NOTHING;
