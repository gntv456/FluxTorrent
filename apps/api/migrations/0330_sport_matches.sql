-- 0330_sport_matches.sql
-- 站型成熟度 · sports 对阵实体（对标 §7.4 BTN season 模型的最后深水区）：
--
-- 「只看湖人对雄鹿」是体育站最基础的检索。0324 已把 league/season/round
-- 维度铺成可筛，但「对阵」是实体不是词表——比分、主客、赛事时间需要
-- 一行一赛的事实表承载。
--
-- 承载（textbook_id 先例：torrents 挂实体外键 + 保留维度检索）：
--   sport_matches：league/season/round + home/away + home_score/away_score
--     + kickoff（开赛时间）+ 状态（未赛/已赛——比分可赛后补录）
--   torrents.match_id BIGINT REFERENCES sport_matches：
--     · 发种可用 match_id 字段挂链（自由字段，同 artifact parent 范式）
--     · 列表按 match 筛（match_id= 参数）；详情聚合带对阵段
--   admin 维护面：POST /admin/matches（建赛+比分）、PUT 比分补录
--   用户读面：GET /matches（按联赛/轮次/队筛）、GET /matches/{id}（对阵页：
--     该场全部过审种——全场/集锦/回放并列）
--
-- 幂等：表/列/索引全部 IF NOT EXISTS。

BEGIN;

CREATE TABLE IF NOT EXISTS sport_matches (
    id BIGSERIAL PRIMARY KEY,
    league TEXT NOT NULL,             -- 与 league 维度词表同名（英超/NBA…）
    season TEXT NOT NULL DEFAULT '',  -- 2025-26
    round TEXT NOT NULL DEFAULT '',   -- 第 10 轮 / 常规赛
    home TEXT NOT NULL,               -- 主队
    away TEXT NOT NULL,               -- 客队
    home_score INT,
    away_score INT,
    kickoff TIMESTAMPTZ,
    -- 已赛=比分非 NULL；未赛/进行中由比分空表达（不做三态枚举，宁简勿猜）
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (league, season, round, home, away)
);

CREATE INDEX IF NOT EXISTS idx_sport_matches_kickoff
    ON sport_matches (kickoff DESC);

ALTER TABLE torrents ADD COLUMN IF NOT EXISTS match_id BIGINT
    REFERENCES sport_matches(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_torrents_match ON torrents (match_id)
    WHERE match_id IS NOT NULL;

COMMIT;
