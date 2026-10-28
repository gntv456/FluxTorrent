-- 0424 同类站对标 A 级批（2026-10-27）：H&R 发布者分成 + 发布者种子级开关 + 月度契约。
--
-- 设计口径（对标报告 _doc/同类站域名-对标分析-2026-10-27.md §4.A）：
--   1) 免罪费分成：同类站 实测免罪 20000 分、发布者得 6000（30%）。我们沿用
--      cancel_hr 价格键，新增 hr_pardon_share_pct（百分比，0=关，缺省 30）。
--      钱只从「已扣的免罪费」里分——不新增铸币口；发布者异常（被禁/删号）或
--      自发种免罪不分成（与 同类站 口径一致）。
--   2) 发布者种子级 HR 开关：hr_policy 已有 enabled 键但只有职员批量口能写。
--      缺省策略不变（站点级 hr_enabled 决定建不建快照），发布者可显式
--      opt-in 开启——开关打开时即便站点缺省是「不追责」也对这颗种子建快照。
--      worker 判侧见 hr_snapshot 的谓词改动（本迁移只加键，判侧 SQL 在
--      worker 侧改，因为建快照 SQL 不在本迁移里）。
--   3) 月度契约（contract）：做种/发种两种，月签月结。表结构对齐
--      seed_preserve 的既有风格（spark_ledger 结算 + 幂等键）。

-- ============ 1) H&R 分成设置键 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('hr_pardon_share_pct', '30',
        'H&R 免罪费给发布者的分成百分比（0=关；其余进系统回收）', 'hr')
ON CONFLICT (name) DO NOTHING;

-- ============ 2) 契约表 ============
-- 一个用户一个月一种契约一行（uk 即幂等闸）。
CREATE SEQUENCE IF NOT EXISTS seed_contracts_id_seq;
CREATE SEQUENCE IF NOT EXISTS seed_contract_tiers_id_seq;

CREATE TABLE IF NOT EXISTS seed_contracts (
    id BIGINT PRIMARY KEY DEFAULT nextval('seed_contracts_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    -- 'seed'（做种体积）| 'publish'（发种数）
    kind TEXT NOT NULL CHECK (kind IN ('seed', 'publish')),
    -- 契约归属月（本地月首日，2026-11-01 表示 2026 年 11 月契约）
    month DATE NOT NULL,
    -- 要求数值：seed=最低平均做种体积字节；publish=最低发种数
    target BIGINT NOT NULL,
    -- 实际达成（月结 job 回填）
    achieved BIGINT NOT NULL DEFAULT 0,
    -- open | settled | breached
    status TEXT NOT NULL DEFAULT 'open',
    -- 结算时间与奖罚
    settled_at TIMESTAMPTZ,
    reward INTEGER NOT NULL DEFAULT 0,
    penalty INTEGER NOT NULL DEFAULT 0,
    auto_renew BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 同类站 口径：本月未完成 → 自动续约保留一次，下下月停。
    -- 连续未达标次数即此计数器。
    missed_streak INT NOT NULL DEFAULT 0,
    CONSTRAINT seed_contracts_uk UNIQUE (user_id, month, kind)
);
CREATE INDEX IF NOT EXISTS seed_contracts_open
    ON seed_contracts (month) WHERE status = 'open';

-- 契约档位（站长可改；用户侧只读展示）。target 与 reward/penalty 一起定义。
CREATE TABLE IF NOT EXISTS seed_contract_tiers (
    id BIGINT PRIMARY KEY DEFAULT nextval('seed_contract_tiers_id_seq'),
    kind TEXT NOT NULL CHECK (kind IN ('seed', 'publish')),
    label TEXT NOT NULL,
    target BIGINT NOT NULL,
    reward INTEGER NOT NULL,
    penalty INTEGER NOT NULL,
    sort INT NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    UNIQUE (kind, target)
);

-- 出厂档位（对齐 同类站 量级做参考；站长后台可调）
INSERT INTO seed_contract_tiers (kind, label, target, reward, penalty, sort) VALUES
    ('seed',    '500GB',  536870912000,  2000, 1000, 1),
    ('seed',    '1TB',   1099511627776,  5000, 2000, 2),
    ('seed',    '2TB',   2199023255552, 12000, 5000, 3),
    ('publish', '5种',   5,               2000, 1000, 1),
    ('publish', '10种',  10,              5000, 2000, 2)
ON CONFLICT DO NOTHING;

-- 契约豁免键（契约期间豁免活跃度考核的天数统计口径）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('seed_contract_enabled', 'yes',
        '月度契约总开关（yes/no）', 'contract')
ON CONFLICT (name) DO NOTHING;
