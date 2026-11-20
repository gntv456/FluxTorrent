-- 0507 运营体检三批：
-- achievement_metrics 指标注册表：成就指标的「metric 名 → SQL 片段」
-- 白名单。此前 worker achv_dormant 三处同款指标 SQL 硬编码（加一个新
-- 成就类型要改 worker 三处），me_achievements 的 CASE 与 arcade_stubs
-- 的 metric_of 又各持一份映射——四处漂移风险。本表收口后：
--   · 新增指标 = INSERT 一行注册表（sql 片段以 {u} 占位用户列），
--     worker 每轮现读注册表动态拼 CTE，零代码改动；
--   · 片段只接受注册表内的固定行（管理员不可写该表，min_class=99 且
--     无后台 CRUD 口），无自由文本进 SQL 的注入面。
-- 播种 = 既有全部 16 个指标（5 社区族 + 10 游玩族 + 集齐类）。

CREATE TABLE IF NOT EXISTS achievement_metrics (
    metric TEXT PRIMARY KEY,
    -- 标量子查询片段，{u} 会被替换为外层 users 别名；只允许注册表
    -- 内的固定行，不接受任何运行时拼接
    sql_expr TEXT NOT NULL,
    descr TEXT NOT NULL DEFAULT '',
    position INT NOT NULL DEFAULT 0
);

INSERT INTO achievement_metrics (metric, sql_expr, descr, position) VALUES
  ('seeding_bytes',  'COALESCE({u}.seeding_size, 0)',
   '做种体积（users.seeding_size）', 1),
  ('rescue_count',   '(SELECT count(*) FROM resurrections r WHERE r.user_id = {u}.id AND r.status = ''done'')',
   '救活种子数', 2),
  ('upload_count',   '(SELECT count(*) FROM torrents t WHERE t.owner_id = {u}.id AND t.approval_status = 1)',
   '过审发种数', 3),
  ('post_count',     '(SELECT count(*) FROM posts p WHERE p.user_id = {u}.id)',
   '论坛帖子数', 4),
  ('subtitle_count', '(SELECT count(*) FROM subtitles sb WHERE sb.user_id = {u}.id AND sb.deleted_at IS NULL AND sb.status = 1 AND NOT sb.anon)',
   '公开过审字幕数', 5),
  ('game_plays',     '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0)',
   '娱乐屋总局数', 6),
  ('scratch_plays',  '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''scratch'')',
   '刮刮乐局数', 7),
  ('jgg_plays',      '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''jgg'')',
   '九宫格局数', 8),
  ('bs_plays',       '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''bigsmall'')',
   '猜大小局数', 9),
  ('farm_water',     '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''farm_water'')',
   '农场浇水次数', 10),
  ('farm_plant',     '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''farm_plant'')',
   '农场播种次数', 11),
  ('capsule_plays',  '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''capsule'')',
   '扭蛋局数', 12),
  ('wheel_plays',    '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''wheel'')',
   '转盘局数', 13),
  ('fishing_plays',  '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''fishing'')',
   '钓鱼次数', 14),
  ('pet_feeds',      '(SELECT count(*) FROM spark_ledger l WHERE l.user_id = {u}.id AND l.kind = ''game'' AND l.amount < 0 AND l.ref_type = ''pet_feed'')',
   '宠物喂养次数', 15)
ON CONFLICT (metric) DO NOTHING;
-- gacha_draws/stub_count 是派生/集齐类（不在注册表：前者查独立表按
-- 人聚合，后者依赖同轮授予结果），worker 代码内特判这两例。

-- arcade_board_kinds 周奖榜单注册表（0507 品类泛化）：kind → label +
-- 榜单 SQL 片段。加新品类 = INSERT 一行 + arcade_week_prize_kinds
-- 开关加词，零代码改动。{week} 占位 = WEEK_SQL 求出的周锚（仅用于
-- 幂等键，片段本身用窗口函数内联）。两行种子与原硬编码逐字同义。
CREATE TABLE IF NOT EXISTS arcade_board_kinds (
    kind TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    -- 单行 SQL：SELECT user_id, <值> AS v ... ORDER BY 值 DESC LIMIT 3
    board_sql TEXT NOT NULL
);

INSERT INTO arcade_board_kinds (kind, label, board_sql) VALUES
  ('lucky', '欧皇',
   'SELECT l.user_id, max(l.amount)::bigint AS v FROM spark_ledger l \
    WHERE l.kind = ''game'' AND l.amount > 0 \
    AND (l.ref_type IN (''scratch'',''bigsmall'',''jgg'',''capsule'',\
    ''wheel'',''fishing'',''farm'') \
    OR l.idempotency_key LIKE ''game-fishing-win:%'') \
    AND l.created_at >= (date_trunc(''week'', \
    (now() AT TIME ZONE ''Asia/Shanghai'') - interval ''7 days'') \
    AT TIME ZONE ''Asia/Shanghai'') \
    AND l.created_at < (date_trunc(''week'', \
    now() AT TIME ZONE ''Asia/Shanghai'') AT TIME ZONE ''Asia/Shanghai'') \
    GROUP BY l.user_id ORDER BY max(l.amount) DESC LIMIT 3'),
  ('fishing', '渔王',
   'SELECT l.user_id, max(l.amount)::bigint AS v FROM spark_ledger l \
    WHERE l.kind = ''game'' AND l.amount > 0 \
    AND l.idempotency_key LIKE ''game-fishing-win:%'' \
    AND l.created_at >= (date_trunc(''week'', \
    (now() AT TIME ZONE ''Asia/Shanghai'') - interval ''7 days'') \
    AT TIME ZONE ''Asia/Shanghai'') \
    AND l.created_at < (date_trunc(''week'', \
    now() AT TIME ZONE ''Asia/Shanghai'') AT TIME ZONE ''Asia/Shanghai'') \
    GROUP BY l.user_id ORDER BY max(l.amount) DESC LIMIT 3')
ON CONFLICT (kind) DO NOTHING;
