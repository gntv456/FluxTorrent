-- 0386 terms 术语中台（PT Studio FR-TERMS-001~030 对标）
--
-- 现状（0205 site_terms + terms.rs）：规则能改、能热加载、正向替换。
-- 但它只是「一张可编辑的替换表」，缺术语中台的**治理三件套**
-- （评估报告 §2.4）：
--
--   1. **统一注册与退役**：`site_terms` 的行是站长随手加的，没有「键」的
--      概念。站长不知道全站一共有多少术语键、哪些已废弃——术语表会
--      慢慢长成一座没人敢动的坟。缺了注册表，就无法回答「这个键还有
--      多少地方在用」；
--   2. **版本快照**：规则改动没有版本，无法回滚、无法 diff。热加载是
--      「立刻生效」，但「立刻生效」也意味着「立刻生效且回不去」；
--   3. **覆盖率巡检**：没有机制回答「术语覆盖了多少界面文案」。
--      现在 i18n 硬编码是老大难，但缺了覆盖率数字，就无法判断
--      「这一批替换到底推进了多少」。
--
-- 【不改变的部分】site_terms 的替换语义与 terms.rs 的热加载完全保留。
-- 本迁移只加治理层：不动既有列、不动既有出口。

-- ① 术语键注册表：全站有哪些键、状态如何
CREATE TABLE IF NOT EXISTS term_keys (
    key         TEXT        PRIMARY KEY,
    -- 站点展示词（zh），registry 用它做可读性
    display_zh  TEXT        NOT NULL DEFAULT '',
    -- 分组：nav / forum / economy / system …
    category    TEXT        NOT NULL DEFAULT 'system',
    -- active=在用 / deprecated=已废弃（仍可渲染，但巡检会告警）
    -- / retired=已退役（不得再被引用）
    status      TEXT        NOT NULL DEFAULT 'active'
                            CHECK (status IN ('active', 'deprecated', 'retired')),
    -- 首次登记与最后改动
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 退役时间：status=retired 时必填，便于回答「什么开始不能用」
    retired_at  TIMESTAMPTZ,
    CHECK ((status = 'retired') = (retired_at IS NOT NULL))
);

COMMENT ON TABLE term_keys IS
    '术语键注册表（0386 治理层）。site_terms 管「怎么替换」，'
    '本表管「有哪些键、还能不能用」。没有它就答不出「这个键还剩'
    '多少引用」，术语表会慢慢变成没人敢动的坟。';

CREATE INDEX IF NOT EXISTS idx_term_keys_status
    ON term_keys (status, category);

-- ② 版本快照：每次改动留档，可 diff 可回滚
CREATE TABLE IF NOT EXISTS term_versions (
    version     BIGSERIAL PRIMARY KEY,
    -- 触发来源：staff（后台改动）/ import（批量导入）/ rollback（回滚）
    source      TEXT        NOT NULL DEFAULT 'staff',
    note        TEXT        NOT NULL DEFAULT '',
    -- 该版本的键值全文（canonical + replacement + enabled）
    snapshot    JSONB       NOT NULL,
    -- 本次改动的键数（用于快速判断「大改」还是「微调」）
    changed     INT         NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by  BIGINT
);

COMMENT ON TABLE term_versions IS
    '术语版本快照（0386）。热加载是「立刻生效」，但没有版本就意味着'
    '「立刻生效且回不去」——改错一个词只能手工改回来。有了版本才有 diff。';

CREATE INDEX IF NOT EXISTS idx_term_versions_time
    ON term_versions (created_at DESC);

-- ③ 巡检报告：覆盖率数字
--    「这一批替换到底推进了多少」——没有数字就只能靠感觉，
--    而 i18n/术语这种长期工程最怕的就是「感觉在推进」。
CREATE TABLE IF NOT EXISTS term_audit_reports (
    id            BIGSERIAL PRIMARY KEY,
    -- 本轮巡检时刻
    audited_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 登记键总数 / 在用 / 已废弃 / 已退役
    total_keys    INT NOT NULL DEFAULT 0,
    active_keys   INT NOT NULL DEFAULT 0,
    deprecated_keys INT NOT NULL DEFAULT 0,
    retired_keys  INT NOT NULL DEFAULT 0,
    -- 有替换规则的键数（= 站长实际配了多少条）
    with_rule     INT NOT NULL DEFAULT 0,
    -- 覆盖率 = with_rule / active_keys（百分比，保留一位小数）
    coverage_pct  NUMERIC(5, 2) NOT NULL DEFAULT 0,
    -- 引用了已退役键的地方（JSONB 数组）——这才是要人处理的东西
    retired_refs  JSONB NOT NULL DEFAULT '[]'::jsonb,
    note          TEXT NOT NULL DEFAULT ''
);

COMMENT ON TABLE term_audit_reports IS
    '术语覆盖率巡检历史。coverage_pct 让「术语中台推进了多少」变成'
    '可量化的数字；retired_refs 是真正需要人处理的那部分——'
    '覆盖率好看但引用着退役键，是典型的假绿。';

CREATE INDEX IF NOT EXISTS idx_term_audit_time
    ON term_audit_reports (audited_at DESC);

-- ④ 版本号视图：取当前最新版本号（乐观锁的 If-Match 用它）
CREATE OR REPLACE VIEW term_current_version AS
SELECT COALESCE(MAX(version), 0) AS version
FROM term_versions;

COMMENT ON VIEW term_current_version IS
    '当前术语版本号。后台改动走乐观锁（客户端回传 If-Match，'
    '不匹配即 409）——这与 0376 版块配置的 expected_mtime 同款纪律。';

-- ⑤ 迁移带种子：把现有 site_terms 的 canonical 登记成 active 键。
--    幂等：ON CONFLICT DO NOTHING，已登记的不动。
INSERT INTO term_keys (key, display_zh, category, status)
SELECT canonical, replacement, 'site', 'active'
FROM site_terms
ON CONFLICT (key) DO NOTHING;
