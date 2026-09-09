-- 0020: 补齐 NexusPHP 对比缺口（高优先 4 项）

CREATE SEQUENCE IF NOT EXISTS resub_uses_id_seq;
CREATE SEQUENCE IF NOT EXISTS password_resets_id_seq;
CREATE SEQUENCE IF NOT EXISTS appeals_id_seq;
CREATE SEQUENCE IF NOT EXISTS hr_violations_id_seq;

-- ============ H&R 追责链路 ============
-- hr_snapshots：完成下载时刻的策略快照（时点正确性：后续改种子配置不影响已完成的追责）
CREATE TABLE IF NOT EXISTS hr_snapshots (
    user_id BIGINT NOT NULL REFERENCES users(id),
    torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
    required_seconds INT NOT NULL,          -- 需做种时长（快照值）
    deadline TIMESTAMPTZ NOT NULL,          -- 免罪截止（completed_at + 宽限期）
    seeded_seconds INT NOT NULL DEFAULT 0,  -- 累计已做种（worker 刷新）
    status TEXT NOT NULL DEFAULT 'open',    -- open | satisfied | violated | pardoned
    pardoned_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, torrent_id)
);
CREATE INDEX IF NOT EXISTS hr_snapshots_open ON hr_snapshots (deadline) WHERE status = 'open';

-- hr_violations：违规记录（追责落库，管理后台 Pardon 用）
CREATE TABLE IF NOT EXISTS hr_violations (
    id BIGINT PRIMARY KEY DEFAULT nextval('hr_violations_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
    seeded_seconds INT NOT NULL,
    required_seconds INT NOT NULL,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,                -- Pardon 时间
    resolved_by BIGINT REFERENCES users(id)
);
CREATE INDEX IF NOT EXISTS hr_violations_unresolved ON hr_violations (user_id) WHERE resolved_at IS NULL;

-- ============ 找回密码 + 邮件通道 ============
CREATE TABLE IF NOT EXISTS password_resets (
    id BIGINT PRIMARY KEY DEFAULT nextval('password_resets_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    token_hash TEXT NOT NULL UNIQUE,        -- sha3-256(token)，明文只在邮件里
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============ 等级自动升降级 ============
-- promotion_rules：晋升条件（运营可改；降级条件 = 不满足最低行）
CREATE TABLE IF NOT EXISTS class_rules (
    class_id INT PRIMARY KEY,
    name TEXT NOT NULL,
    min_uploaded BIGINT NOT NULL DEFAULT 0,      -- 字节
    min_download_count INT NOT NULL DEFAULT 0,   -- 完成下载数
    min_seed_hours INT NOT NULL DEFAULT 0,       -- 累计做种小时
    min_account_age_days INT NOT NULL DEFAULT 0,
    demotable BOOLEAN NOT NULL DEFAULT TRUE      -- 不满足时是否自动降级（LV1 保底不降）
);

INSERT INTO class_rules (class_id, name, min_uploaded, min_download_count, min_seed_hours, min_account_age_days, demotable) VALUES
    (1, 'LV1 新芽', 0, 0, 0, 0, FALSE),
    (2, 'LV2 幼苗', 53687091200, 5, 72, 7, TRUE),      -- 50GB / 5种 / 72h / 7天
    (3, 'LV3 小树', 268435456000, 20, 240, 30, TRUE),  -- 250GB / 20种 / 240h / 30天
    (4, 'LV4 大树', 1073741824000, 50, 720, 90, TRUE), -- 1TB / 50种 / 720h / 90天
    (5, 'LV5 森林', 4294967296000, 120, 1440, 180, TRUE),
    (6, 'LV6 生态', 10737418240000, 300, 2880, 365, TRUE)
ON CONFLICT (class_id) DO UPDATE SET
    name = EXCLUDED.name, min_uploaded = EXCLUDED.min_uploaded,
    min_download_count = EXCLUDED.min_download_count,
    min_seed_hours = EXCLUDED.min_seed_hours,
    min_account_age_days = EXCLUDED.min_account_age_days,
    demotable = EXCLUDED.demotable;

-- ============ 申诉系统 ============
CREATE TABLE IF NOT EXISTS appeals (
    id BIGINT PRIMARY KEY DEFAULT nextval('appeals_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    kind TEXT NOT NULL,                     -- hr | warn | ban | other
    ref_id BIGINT,                          -- 关联对象（如 hr_violations.id）
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open',    -- open | accepted | rejected
    handled_by BIGINT REFERENCES users(id),
    handled_at TIMESTAMPTZ,
    result_note TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS appeals_open ON appeals (created_at) WHERE status = 'open';

-- ============ 附件/截图 ============
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS screenshots JSONB;   -- ["https://..."]

-- IMDb/pt_gen 信息区（详情页富信息）
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS media_info JSONB;    -- {imdb_id, rating, year, directors, ...}

-- ============ 补签卡道具 ============
-- shop_items 里已有「补签卡」（item 12），效果标记 kind='resub_card'；
-- 这里建使用记录（一卡一用）
CREATE TABLE IF NOT EXISTS resub_uses (
    id BIGINT PRIMARY KEY DEFAULT nextval('resub_uses_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    target_date DATE NOT NULL,              -- 补签目标日
    idempotency_key TEXT NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
