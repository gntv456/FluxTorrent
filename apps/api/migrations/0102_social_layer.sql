-- 0102：社交层 —— 组队契约 / 赛季 / 信誉 / 拯救荣誉。
--
-- 设计前提（重要）：FluxTorrent 已有一套完整的资源抢救体系，本迁移**只做加法，不另起炉灶**：
--
--   已有（0073，U3D Graveyard 口径）：
--     resurrections 表          torrent_id 唯一 / user_id / required_hours / reward_sparks / status(open|done|expired)
--     GET  /resurrections       死种列表（seeders=0 + 30 天无 snatches 活动 + 无未完成任务）
--     POST /resurrections/claim 认领（不能领自己的种、CAS 防双领、required_hours 领取时快照）
--     GET  /resurrections/mine  我的任务
--     worker 每小时验收 → 5000 火花 + 1 枚免费券 + 该种 7 天免费
--
--   已有（做种激励 / 保种 / 账本 / 反作弊，全部复用，本迁移不碰）：
--     jobs.rs::seeding_reward（规则分 × seeders^-0.35 × 时长半衰 + arctan 软封顶 + 反假保种）
--     jobs.rs::preserve_settle / preserve_exit（seed_preserve 基线 delta 口径）
--     spark_ledger（分区表，幂等键应用层先查后插）
--     cheat_events + users.status
--
-- 因此本迁移的增量只有三类：
--   ① 组队：给 resurrections 加 team_id（NULL = 现有单人任务，向后完全兼容）
--   ② 赛季：周期性重置的排名与发行池
--   ③ 信誉与荣誉：跨季保留的长期资产
--
-- 注：契约成员贡献不使用 spark_ledger 反查（seeding_reward 按用户逐小时聚合落库、无 ref_id，
--     无法定位到单个种子），改为在 snatches 上记基线、结算时算增量 —— 与 seed_preserve 同款口径。

-- ============ ① 组队：把单人复活任务扩展为多人契约 ============

CREATE TABLE IF NOT EXISTS social_team (
    id            BIGSERIAL PRIMARY KEY,
    leader_uid    BIGINT NOT NULL REFERENCES users(id),
    season_id     BIGINT,                                  -- 引用 social_season，见下方建表后补 FK
    title         TEXT NOT NULL DEFAULT '',
    status        SMALLINT NOT NULL DEFAULT 0,             -- 0招募 1进行 2已完成 3失败 4取消
    team_size_max INT NOT NULL DEFAULT 5,                  -- 下限恒为 1：人数是加成，不是门槛
    settled_at    TIMESTAMPTZ,
    fail_reason   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_social_team_leader ON social_team (leader_uid, status);

-- 关联到已有的复活任务：team_id 为 NULL 时是现有的单人任务，行为完全不变
ALTER TABLE resurrections ADD COLUMN IF NOT EXISTS team_id BIGINT
    REFERENCES social_team(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_resurrections_team ON resurrections (team_id);

-- 契约成员：贡献基线取自 snatches（该成员对该契约目标资源的累计值）
CREATE TABLE IF NOT EXISTS social_team_member (
    team_id            BIGINT NOT NULL REFERENCES social_team(id) ON DELETE CASCADE,
    uid                BIGINT NOT NULL REFERENCES users(id),
    role_key           TEXT NOT NULL DEFAULT 'keeper',     -- 岗位，取值由主题包定义
    is_leader          BOOLEAN NOT NULL DEFAULT FALSE,
    join_status        SMALLINT NOT NULL DEFAULT 0,        -- 0邀请 1接受 2拒绝 3退出 4完成
    specialty_lv       INT NOT NULL DEFAULT 0,             -- 岗位专精（赛季内累计，跨季重置）
    seed_seconds_begin BIGINT NOT NULL DEFAULT 0,
    uploaded_begin     BIGINT NOT NULL DEFAULT 0,
    contributed_sec    BIGINT NOT NULL DEFAULT 0,          -- 结算时回填的做种增量（秒）
    settled_amount     BIGINT NOT NULL DEFAULT 0,          -- 本契约分账（写入 spark_ledger 后回填）
    joined_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    left_at            TIMESTAMPTZ,
    PRIMARY KEY (team_id, uid)
);
CREATE INDEX IF NOT EXISTS idx_social_team_member_uid ON social_team_member (uid, join_status);

-- ============ ② 赛季 ============

CREATE TABLE IF NOT EXISTS social_season (
    id             BIGSERIAL PRIMARY KEY,
    seq_no         INT NOT NULL UNIQUE,                    -- 第几季，从 1 起
    theme_code     TEXT NOT NULL,                          -- 复用 site_type_packs.code
    name           TEXT NOT NULL,
    status         SMALLINT NOT NULL DEFAULT 0,            -- 0预告 1进行 2结算中 3已结算
    starts_at      TIMESTAMPTZ NOT NULL,
    ends_at        TIMESTAMPTZ NOT NULL,
    readonly_until TIMESTAMPTZ,                            -- 结算后只读期截止
    pool_cap       BIGINT NOT NULL DEFAULT 0,              -- 发行硬顶；0 = 按通胀公式自动计算
    pool_coef      NUMERIC(6,3) NOT NULL DEFAULT 1.000,    -- 次线性发行系数 k，每季按上季数据重算
    issued_amount  BIGINT NOT NULL DEFAULT 0,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_social_season_status ON social_season (status, starts_at DESC);

-- 补 social_team.season_id 外键（建表顺序所限，此处补上）
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'social_team_season_fk') THEN
        ALTER TABLE social_team ADD CONSTRAINT social_team_season_fk
            FOREIGN KEY (season_id) REFERENCES social_season(id) ON DELETE SET NULL;
    END IF;
END $$;

-- ============ ③ 信誉（跨季按 social_reputation_keep_ratio 衰减）============
-- 事实驱动为主（任务完成/失败/退出，系统判定不可伪造），队友评价权重 <= 10% 且每季有配额，防互刷
CREATE TABLE IF NOT EXISTS social_reputation (
    uid             BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    score           BIGINT NOT NULL DEFAULT 1000,
    fulfilled_count INT NOT NULL DEFAULT 0,
    withdrawn_count INT NOT NULL DEFAULT 0,
    failed_count    INT NOT NULL DEFAULT 0,
    review_quota    INT NOT NULL DEFAULT 5,                -- 本季剩余评价配额
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============ ③ 拯救荣誉（永久，不随赛季重置）============
-- 单独建表的理由：resurrections 对 torrent_id 是 ON DELETE CASCADE，种子被删则任务记录一并消失；
-- 荣誉必须独立留存（这也是「谁救过什么」唯一不会被抹掉的记录）。
CREATE TABLE IF NOT EXISTS rescue_honor (
    id             BIGSERIAL PRIMARY KEY,
    torrent_id     BIGINT NOT NULL,                        -- 故意不加 FK
    info_hash      CHAR(40),
    team_id        BIGINT REFERENCES social_team(id) ON DELETE SET NULL,
    season_id      BIGINT REFERENCES social_season(id) ON DELETE SET NULL,
    uids           BIGINT[] NOT NULL,                      -- 只记真实用户，无 NPC 占位
    total_sec      BIGINT NOT NULL DEFAULT 0,              -- 契约期团队做种总时长（文案如实展示，不夸大）
    seeders_before INT NOT NULL DEFAULT 0,
    seeders_after  INT NOT NULL DEFAULT 0,
    rescued_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_rescue_honor_torrent ON rescue_honor (torrent_id);
CREATE INDEX IF NOT EXISTS idx_rescue_honor_uids ON rescue_honor USING GIN (uids);

-- ============ 配置项 ============

-- 总开关：默认关闭（通用程序不应默认暴露社区玩法；关闭时接口返回 enabled=false，前端隐藏入口）
INSERT INTO site_settings (name, value, grp, descr) VALUES
    ('module_social', 'no', 'main', '开放社交层（濒危预警 / 组队契约 / 赛季）')
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, grp, descr) VALUES
    ('social_endangered_seeders',     '1',    'main', '濒危预警阈值：做种数 <= 该值即列入预警雷达'),
    ('social_health_seeders',         '7',    'main', '资源「健康」做种数阈值（对齐保种区移出口径）'),
    ('social_season_weeks',           '4',    'main', '赛季长度（周）'),
    ('social_pool_coef',              '1.0',  'main', '赛季发行次线性系数 k'),
    ('social_pool_cap',               '0',    'main', '赛季发行硬顶（火花）；0 = 按通胀公式自动计算'),
    ('social_income_ratio_cap',       '0.30', 'main', '社交层加成上限：不得超过基础做种收益的 30%'),
    ('social_readonly_hours',         '24',   'main', '赛季结算后只读期（小时）'),
    ('social_reputation_keep_ratio',  '0.70', 'main', '信誉跨季保留比例'),
    ('social_min_days_to_season_end', '7',    'main', '距赛季结束不足该天数时禁止发起新契约'),
    ('social_review_quota',           '5',    'main', '每季队友评价配额（防互刷）')
ON CONFLICT (name) DO NOTHING;
