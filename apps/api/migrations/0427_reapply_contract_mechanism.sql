-- 0427 把 同类站 机制层（0424/0425）**真正建出来**（P-461，2026-10-28 现网取证）。
--
-- 现象（当场数过，非推断）：
--   1) `_sqlx_migrations` 里 424/425 记着 applied（success=t，
--      installed_on 2026-10-27 18:53），且**记录的 checksum 与现在磁盘上的
--      0424/0425 逐字节一致**——所以这不是「跑过之后文件又被改」（P-442 那一型），
--      而是**对象从来没建过**：
--        seed_contracts / seed_contract_tiers / search_term_stats 三张表：无
--        seeding_publisher_mult() 函数：无（同库 pg_proc 里老函数
--        u_notice_enabled 在，查询本身有效，不是看不见）
--        hr_pardon_share_pct / seed_contract_enabled /
--        seeding_publisher_mult_{day,month,year} / upload_either_or：全缺
--   2) 后果已经在界面上：`GET /api/v1/contracts/tiers` 与 `/contracts/mine`
--      现网各回 **500 code=1000**；worker 侧 `dormant_mark` 的
--      `job_status.last_ok = f`；`jobs/seeding.rs:69` 每 60s 就要调一次
--      `seeding_publisher_mult(...)`、`jobs/contract.rs` 与
--      `achv_dormant.rs:220` 每轮都要读 `seed_contracts`。
--   3) 最阴的一条：`torrent_http/suggest.rs:83` 往 search_term_stats 写计数时
--      是 `let _ = sqlx::query(...)`——表不存在被**静默吞掉**，热搜不是报错，
--      而是永远零累积。
--
-- 根因是两条，都是当场跑出来的，不是读码猜的：
--   R1 **0425 的 DDL 在 PostgreSQL 上根本执行不了**：结尾那条部分索引
--      `CREATE INDEX ... WHERE last_at > now() - interval '90 days'` 的谓词
--      用了 now()，而 `now()` 是 STABLE 不是 IMMUTABLE ⇒
--      `ERROR: functions in index predicate must be marked IMMUTABLE`。
--      `scripts/pt_ci_migration_chain.py` 在干净空库按序跑全链，实测停在
--      第 419/421 条（就是 0425）——sqlx 是单链，这一条炸 ⇒ 它之后所有迁移
--      （0426、本文件…）在**任何新站**都不会执行。所以本文件的同名索引改成
--      普通索引：90 天窗口是读侧的查询条件（home.rs 那条 SELECT 自己带
--      `WHERE last_at > now() - interval '90 days'`），不需要、也不能进谓词。
--   R2 现网这两行台账**不是 sqlx 写的，是 `scripts/record_migration.py` 手工
--      登记的**，签名两条：`execution_time = 0`（sqlx 真跑的行是微秒数，
--      同一库里 420=18167839、426=12022146），description 带着 4 位版本号前缀
--      （sqlx 写的不带）。那件工具的原文只算 checksum 就落行，把「登记」当成
--      「已执行」的同义词，于是 0425 报错也照样记成 applied——而 sqlx 只按
--      版本号跳过，这笔假账此后**永远不会自愈**。已给那件工具补上对象核验。
--      对照件：0423 是同一把工具登记的，但它的对象确实在库里
--      （`promotions.announced_at` 存在）⇒ 判据只能钉「对象在不在」，
--      不能钉「谁写的行」。
--
-- 为什么不能改 0425 的文件内容（R1 的正解要等站长拍板）：台账已有 425 行，
-- sqlx 启动时比对 checksum，字节一变就是
-- `migration 425 was previously applied but has been modified` ⇒ api 启动即退
-- （本仓被这一型咬过两次）。改 0425 必须连着现网那行假账一起 reconcile。
--
-- 为什么必须开新号（而不是改 0424/0425）：sqlx 只按版本号记账，已记为 applied
-- 的文件下次启动直接跳过，改文件内容对**存量站永远不可见**（P-442 的机制，
-- 本仓已为此立过 LV 闸门）。新环境里 0424/0425 会正常建，所以本文件的每一条
-- 都必须是幂等的：IF NOT EXISTS / ON CONFLICT DO NOTHING /
-- CREATE OR REPLACE FUNCTION——两边跑都不炸、不重复。

-- ============ 1) H&R 分成 + 契约开关（原 0424） ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('hr_pardon_share_pct', '30',
        'H&R 免罪费给发布者的分成百分比（0=关；其余进系统回收）', 'hr')
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('seed_contract_enabled', 'yes',
        '月度契约总开关（yes/no）', 'contract')
ON CONFLICT (name) DO NOTHING;

CREATE SEQUENCE IF NOT EXISTS seed_contracts_id_seq;
CREATE SEQUENCE IF NOT EXISTS seed_contract_tiers_id_seq;

CREATE TABLE IF NOT EXISTS seed_contracts (
    id BIGINT PRIMARY KEY DEFAULT nextval('seed_contracts_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    -- 'seed'（做种体积）| 'publish'（发种数）
    kind TEXT NOT NULL CHECK (kind IN ('seed', 'publish')),
    -- 契约归属月（本地月首日，2026-11-01 表示 2026 年 11 月契约）
    month DATE NOT NULL,
    target BIGINT NOT NULL,
    -- 实际达成（月结 job 回填）
    achieved BIGINT NOT NULL DEFAULT 0,
    -- open | settled | breached
    status TEXT NOT NULL DEFAULT 'open',
    settled_at TIMESTAMPTZ,
    reward INTEGER NOT NULL DEFAULT 0,
    penalty INTEGER NOT NULL DEFAULT 0,
    auto_renew BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 同类站 口径：本月未完成 → 自动续约保留一次，下下月停
    missed_streak INT NOT NULL DEFAULT 0,
    CONSTRAINT seed_contracts_uk UNIQUE (user_id, month, kind)
);
CREATE INDEX IF NOT EXISTS seed_contracts_open
    ON seed_contracts (month) WHERE status = 'open';

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

INSERT INTO seed_contract_tiers (kind, label, target, reward, penalty, sort)
VALUES
    ('seed',    '500GB',  536870912000,  2000, 1000, 1),
    ('seed',    '1TB',   1099511627776,  5000, 2000, 2),
    ('seed',    '2TB',   2199023255552, 12000, 5000, 3),
    ('publish', '5种',   5,               2000, 1000, 1),
    ('publish', '10种',  10,              5000, 2000, 2)
ON CONFLICT DO NOTHING;

-- ============ 2) 发布者三档衰减 + 弹性校验 + 热搜计数（原 0425） ============
INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('seeding_publisher_mult_day',  '4.0',
   '发布者为自发种做种：种龄<24h 的加成倍数（1.0=关）', 'seeding'),
  ('seeding_publisher_mult_month','2.5',
   '发布者为自发种做种：种龄<30天 的加成倍数（1.0=关）', 'seeding'),
  ('seeding_publisher_mult_year', '1.5',
   '发布者为自发种做种：种龄<365天 的加成倍数（1.0=关）', 'seeding')
ON CONFLICT (name) DO NOTHING;

CREATE OR REPLACE FUNCTION seeding_publisher_mult(
    p_is_publisher boolean,
    p_torrent_age_days double precision
) RETURNS double precision LANGUAGE sql STABLE AS $$
    SELECT CASE
        WHEN NOT p_is_publisher THEN 1.0
        WHEN p_torrent_age_days < 1.0 THEN
            COALESCE((SELECT value FROM site_settings
                      WHERE name = 'seeding_publisher_mult_day')::double precision, 4.0)
        WHEN p_torrent_age_days < 30.0 THEN
            COALESCE((SELECT value FROM site_settings
                      WHERE name = 'seeding_publisher_mult_month')::double precision, 2.5)
        WHEN p_torrent_age_days < 365.0 THEN
            COALESCE((SELECT value FROM site_settings
                      WHERE name = 'seeding_publisher_mult_year')::double precision, 1.5)
        ELSE 1.0
    END
$$;

COMMENT ON FUNCTION seeding_publisher_mult IS
    '发布者做种三档衰减乘数（0425 同类站对标，0427 补建）：仅对 owner 自己的'
    '种子生效，档位随种龄衰减，1.0=关。';

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('upload_either_or', 'no',
        '发种内容门槛四选一（yes=简介≥50字/有截图/有MediaInfo/'
        '有IMDb豆瓣TMDB任一达标即过）',
        'upload')
ON CONFLICT (name) DO NOTHING;

CREATE TABLE IF NOT EXISTS search_term_stats (
    term TEXT PRIMARY KEY,
    hits BIGINT NOT NULL DEFAULT 0,
    last_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- 0425 原文这里是 `WHERE last_at > now() - interval '90 days'` 的**部分索引**，
-- 而 `now()` 是 STABLE 不是 IMMUTABLE ⇒ PostgreSQL 直接拒绝：
-- `ERROR: functions in index predicate must be marked IMMUTABLE`。
-- 这就是 0425 从来没跑成功过的原因（干净库跑链在第 419/421 条中止，
-- scripts/pt_ci_migration_chain.py 实测）。这里改成普通索引：90 天窗口是
-- **查询条件**（热搜取数那边已经带 `last_at > now() - interval '90 days'`），
-- 不需要、也不能进索引谓词。热搜表体量是个位数千行，普通索引足够。
CREATE INDEX IF NOT EXISTS search_term_stats_hot
    ON search_term_stats (hits DESC, last_at DESC);
