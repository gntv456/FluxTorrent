-- 娱乐屋「大厅游戏表面」：票根册 + 周常/赛季领取幂等 + 确定侧预算配置（2026-09-29）
-- 设计要点（见 _doc/娱乐屋大厅游戏表面移植-开发计划-2026-09-29.md）：
--   1. 票根册**复用既有成就设施**（achievement_defs 增 family='arcade' + user_achievements 记持有），
--      不新建收集表。metric 由 games_http::arcade_meta 从 spark_ledger(kind='game') 实时聚合——
--      worker 的 achievement_grant 只认 5 个老 metric（seeding_bytes/post_count/...），
--      不为它加 game 分支（那会让 3 处重复 SQL 再膨胀一圈），改由大厅端点按需算。
--   2. 周常/赛季领取走 arcade_claims 幂等表（PK 含 period_key：周常='2026-W40'、赛季='S1'），
--      重复提交 / 定时任务重跑安全。
--   3. 确定侧发放预算参数落 site_settings（arcade_budget_*）——EV 闸只管随机侧，
--      周常/赛季这类确定发放必须另有一道闸，否则「周常送免考核卡」就是绕过奖池守卫的侧门。

-- ── 12 张票根（arcade 成就）──
-- metric 仅用可从玩法流水派生的口径：game_plays / scratch_plays / jgg_plays / farm_water /
-- farm_plant / stub_count（集齐计数）。不引入需要新增埋点的 metric。
INSERT INTO achievement_defs (family, code, name, descr, metric, threshold, reward_sparks, position) VALUES
  ('arcade', 'arc_first',     '旧铜票根',    '首次开玩任意玩法',         'game_plays',    1,   0, 1),
  ('arcade', 'arc_scratch_12','刮开十二张',  '累计刮开 12 张刮刮乐',     'scratch_plays', 12,  0, 2),
  ('arcade', 'arc_jgg_30',    '整点同板首抽','累计九宫格抽满 30 次',     'jgg_plays',     30,  0, 3),
  ('arcade', 'arc_farm_3',    '丰收季 · 春', '累计浇水 3 次',            'farm_water',    3,   0, 4),
  ('arcade', 'arc_bs_10',     '三倍夜',      '猜大小累计 10 局',         'bs_plays',      10,  0, 5),
  ('arcade', 'arc_farm_30',   '满园',        '累计浇水 30 次',           'farm_water',    30,  0, 6),
  ('arcade', 'arc_scratch_100','限定 · 虹膜','累计刮开 100 张刮刮乐',    'scratch_plays', 100, 0, 7),
  ('arcade', 'arc_plays_200', '游园常客',    '累计游玩 200 局',          'game_plays',    200, 0, 8),
  ('arcade', 'arc_jgg_100',   '锁 4 连达成', '累计九宫格抽满 100 次',    'jgg_plays',     100, 0, 9),
  ('arcade', 'arc_farm_100',  '大数连龙 7',  '累计浇水 100 次',          'farm_water',    100, 0, 10),
  ('arcade', 'arc_plays_500', '票根册 · 首章','累计游玩 500 局',         'game_plays',    500, 0, 11),
  ('arcade', 'arc_all_star',  '站长亲授',    '集齐其余 11 张票根',       'stub_count',    11,  0, 12)
ON CONFLICT (code) DO NOTHING;

-- ── 周常/赛季领取幂等表 ──
CREATE TABLE IF NOT EXISTS arcade_claims (
    user_id    bigint      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind       text        NOT NULL,   -- 'quest' | 'season'
    ref_code   text        NOT NULL,   -- 任务/里程碑 code
    period_key text        NOT NULL,   -- 周常 '2026-W40'；赛季 'S1'
    claimed_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, kind, ref_code, period_key)
);

COMMENT ON TABLE arcade_claims IS
    '娱乐屋周常/赛季领取幂等记录（PK 含 period_key，重复提交与定时任务重跑安全）';

-- ── 确定侧预算与开关（site_settings）──
INSERT INTO site_settings (name, value, grp, descr) VALUES
  ('arcade_budget_base',       '1500', 'games', '娱乐屋确定侧发放预算基数'),
  ('arcade_budget_pct',        '20',   'games', '确定侧预算 = 基数 + 窗口内回收 × 该百分比'),
  ('arcade_budget_window_days','7',    'games', '确定侧预算的回收窗口（天）')
ON CONFLICT (name) DO NOTHING;

-- 玩法流水按用户+时间聚合加速（纹章预算环 / 周常进度 / 真回收均走此查询）
CREATE INDEX IF NOT EXISTS idx_spark_game_user_time
    ON spark_ledger (user_id, created_at)
    WHERE kind = 'game';

COMMENT ON INDEX idx_spark_game_user_time IS
    '娱乐屋大厅：按用户+窗口聚合玩法流水（真回收 / 周常进度）';
