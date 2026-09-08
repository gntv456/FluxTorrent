-- FluxTorrent 初始 schema（方案 §6.2）
CREATE EXTENSION IF NOT EXISTS citext;
-- 迁移纪律（§8.5-5）：只增不改；破坏性变更走三步。

-- ============ 枚举与基础表 ============

CREATE TABLE user_classes (
  id INT PRIMARY KEY,
  name TEXT NOT NULL,
  pod_level INT NOT NULL,              -- 成长豆荚 LV0-6
  min_uploaded BIGINT NOT NULL DEFAULT 0,
  min_ratio NUMERIC NOT NULL DEFAULT 0,
  min_age_days INT NOT NULL DEFAULT 0,
  privileges JSONB NOT NULL DEFAULT '{}'
);

INSERT INTO user_classes (id, name, pod_level) VALUES
  (0, '种子', 0), (1, '新芽', 1), (2, '幼苗', 2), (3, '小树', 3),
  (4, '大树', 4), (5, '开花', 5), (6, '硕果', 6),
  (90, '维护开发员', 90), (91, '发布员', 91), (92, '论坛版主', 92),
  (93, '总版主', 93), (94, '管理员', 94), (95, '主管', 95), (99, '站长', 99);

CREATE TABLE users (
  id BIGSERIAL PRIMARY KEY,
  username CITEXT NOT NULL UNIQUE,
  email CITEXT NOT NULL UNIQUE,
  pass_hash TEXT NOT NULL,
  passkey CHAR(32) NOT NULL UNIQUE,
  class_id INT NOT NULL DEFAULT 0 REFERENCES user_classes(id),
  uploaded BIGINT NOT NULL DEFAULT 0,
  downloaded BIGINT NOT NULL DEFAULT 0,
  spark_balance BIGINT NOT NULL DEFAULT 0,
  title TEXT,
  avatar_url TEXT,
  invited_by BIGINT REFERENCES users(id),
  must_reset_password BOOLEAN NOT NULL DEFAULT FALSE,
  donor BOOLEAN NOT NULL DEFAULT FALSE,
  status SMALLINT NOT NULL DEFAULT 0,   -- 0正常 1禁言 2封号
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_seen_at TIMESTAMPTZ
);

CREATE TABLE invites (
  id BIGSERIAL PRIMARY KEY,
  inviter_id BIGINT NOT NULL REFERENCES users(id),
  code CHAR(32) NOT NULL UNIQUE,
  status SMALLINT NOT NULL DEFAULT 0,   -- 0未用 1已用 2过期 3撤销
  used_by BIGINT REFERENCES users(id),
  expires_at TIMESTAMPTZ NOT NULL
);

-- ============ 种子域 ============

CREATE TABLE categories (
  id INT PRIMARY KEY,                   -- 1-7 对应学前/小学/初中/职高/高中/教育影音/纪录片（旧站 cat401-410 归一）
  name TEXT NOT NULL,
  legacy_id INT                         -- 旧站 cat401..  映射留存
);
INSERT INTO categories (id, name, legacy_id) VALUES
  (1,'学前教育',401),(2,'小学',402),(3,'初中',403),(4,'职高',404),
  (5,'高中',405),(6,'教育影音',406),(7,'纪录片',407);

CREATE TABLE media (id INT PRIMARY KEY, name TEXT NOT NULL);
INSERT INTO media (id, name) VALUES
  (1,'视频'),(2,'音频'),(3,'书籍'),(4,'文档'),(5,'笔记'),(6,'课件'),(7,'软件'),(8,'图片');

CREATE TABLE grades (id INT PRIMARY KEY, name TEXT NOT NULL);
INSERT INTO grades (id, name) VALUES
  (0,'幼儿园'),(1,'一年级'),(2,'二年级'),(3,'三年级'),(4,'四年级'),(5,'五年级'),
  (6,'六年级'),(7,'初一'),(8,'初二'),(9,'初三'),(10,'高一'),(11,'高二'),(12,'高三');

CREATE TABLE editions (id INT PRIMARY KEY, name TEXT NOT NULL);
INSERT INTO editions (id, name) VALUES
  (1,'人教'),(2,'部编'),(3,'统编'),(4,'苏教'),(5,'北师大'),(6,'外研'),(7,'沪教');

CREATE TABLE textbooks (
  id BIGSERIAL PRIMARY KEY,
  subject TEXT NOT NULL,
  edition_id INT NOT NULL REFERENCES editions(id),
  grade_id INT NOT NULL REFERENCES grades(id),
  volume TEXT,
  publisher TEXT,
  downloads INT NOT NULL DEFAULT 0
);

CREATE TABLE torrents (
  id BIGSERIAL PRIMARY KEY,
  info_hash CHAR(40) NOT NULL UNIQUE,
  name TEXT NOT NULL,
  small_descr TEXT,
  descr TEXT,
  technical_info TEXT,
  nfo TEXT,
  category_id INT NOT NULL REFERENCES categories(id),
  medium_id INT NOT NULL REFERENCES media(id),
  grade_id INT REFERENCES grades(id),
  edition_id INT REFERENCES editions(id),
  textbook_id BIGINT REFERENCES textbooks(id),
  owner_id BIGINT REFERENCES users(id),
  anonymous BOOLEAN NOT NULL DEFAULT FALSE,
  size BIGINT NOT NULL,
  numfiles INT NOT NULL DEFAULT 0,
  approval_status SMALLINT NOT NULL DEFAULT 0,  -- 0待审 1通过 2拒绝
  sticky BOOLEAN NOT NULL DEFAULT FALSE,
  official_tag BOOLEAN NOT NULL DEFAULT FALSE,
  hr_policy JSONB,
  seeders INT NOT NULL DEFAULT 0,
  leechers INT NOT NULL DEFAULT 0,
  times_completed INT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  mtime TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 列表页覆盖索引（方案 §6.2：游标分页 + 覆盖索引）
CREATE INDEX idx_torrents_list ON torrents
  (created_at DESC, id, name, small_descr, size, category_id,
   seeders, leechers, times_completed, official_tag);
CREATE INDEX idx_torrents_filter ON torrents (category_id, medium_id, grade_id, edition_id, created_at DESC);
CREATE INDEX idx_torrents_approval ON torrents (approval_status, created_at DESC);

CREATE TABLE tags (
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  tag_id INT NOT NULL,                       -- 官种=3（旧站口径）
  PRIMARY KEY (torrent_id, tag_id)
);

CREATE TABLE files (
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  file_index INT NOT NULL,
  path TEXT NOT NULL,
  size BIGINT NOT NULL,
  PRIMARY KEY (torrent_id, file_index)
);

CREATE TABLE comments (
  id BIGSERIAL PRIMARY KEY,
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  body TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  edited_at TIMESTAMPTZ
);

CREATE TABLE thanks (
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (torrent_id, user_id)
);

CREATE TABLE bookmarks (
  user_id BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, torrent_id)
);

CREATE TABLE snatches (
  user_id BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  uploaded BIGINT NOT NULL DEFAULT 0,
  downloaded BIGINT NOT NULL DEFAULT 0,
  seeded_seconds INT NOT NULL DEFAULT 0,
  completed_at TIMESTAMPTZ,
  leeching BOOLEAN NOT NULL DEFAULT FALSE,
  seeding BOOLEAN NOT NULL DEFAULT FALSE,
  hr_flag BOOLEAN NOT NULL DEFAULT FALSE,
  PRIMARY KEY (user_id, torrent_id)
);

-- ============ 促销与计费（M06 / §5.4） ============

CREATE TYPE promotion_scope AS ENUM ('torrent', 'global');
CREATE TYPE promotion_kind_enum AS ENUM ('free','x2','x2free','half','x2half','p30');
CREATE TYPE promotion_source AS ENUM ('manual','magic_pool','preserve_grace','task');

CREATE TABLE promotions (
  id BIGSERIAL PRIMARY KEY,
  scope promotion_scope NOT NULL,
  torrent_id BIGINT REFERENCES torrents(id) ON DELETE CASCADE,
  kind promotion_kind_enum NOT NULL,
  starts_at TIMESTAMPTZ NOT NULL,
  ends_at TIMESTAMPTZ NOT NULL,
  source promotion_source NOT NULL DEFAULT 'manual',
  created_by BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK ((scope = 'global') = (torrent_id IS NULL))
);
CREATE INDEX idx_promotions_active ON promotions (scope, torrent_id, ends_at);

-- 计费流水：按月分区（分区表 + 继承默认分区，自动建分区由 worker 负责）
CREATE SEQUENCE traffic_ledger_id_seq;
CREATE TABLE traffic_ledger (
  id BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  torrent_id BIGINT NOT NULL,
  delta_up BIGINT NOT NULL,
  delta_down BIGINT NOT NULL,
  promotion_kind SMALLINT NOT NULL DEFAULT 0,
  window_start TIMESTAMPTZ NOT NULL,
  ingested_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, id, window_start)
) PARTITION BY RANGE (window_start);
CREATE TABLE traffic_ledger_default PARTITION OF traffic_ledger DEFAULT;

-- ============ 经济域（M11-M13） ============

CREATE SEQUENCE spark_ledger_id_seq;
CREATE TABLE spark_ledger (
  id BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  amount BIGINT NOT NULL,
  kind TEXT NOT NULL,
  ref_type TEXT,
  ref_id BIGINT,
  idempotency_key TEXT,
  balance_after BIGINT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, id, created_at)
) PARTITION BY RANGE (created_at);
-- 分区键限制：idempotency_key 无法建全局唯一约束，幂等由应用层「先查后插」保证（§8.5 迁移纪律权衡记录）
CREATE INDEX idx_spark_idem ON spark_ledger (idempotency_key);
CREATE TABLE spark_ledger_default PARTITION OF spark_ledger DEFAULT;

CREATE TABLE bank_deposits (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  amount BIGINT NOT NULL,
  term_days INT NOT NULL,
  rate NUMERIC NOT NULL,
  interest BIGINT NOT NULL DEFAULT 0,
  status SMALLINT NOT NULL DEFAULT 0,
  start_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  maturity_at TIMESTAMPTZ NOT NULL,
  settled_at TIMESTAMPTZ
);

CREATE TABLE shop_items (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL,
  price BIGINT NOT NULL,
  config JSONB NOT NULL DEFAULT '{}',
  active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE shop_orders (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  item_id BIGINT NOT NULL REFERENCES shop_items(id),
  price BIGINT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  config_snapshot JSONB,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE magic_pool (
  month CHAR(7) PRIMARY KEY,
  donated_total BIGINT NOT NULL DEFAULT 0,
  goal BIGINT NOT NULL DEFAULT 2000000,
  promo_started BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE TABLE attendance (
  user_id BIGINT NOT NULL REFERENCES users(id),
  date DATE NOT NULL,
  streak INT NOT NULL DEFAULT 1,
  reward BIGINT NOT NULL DEFAULT 0,
  makeup BOOLEAN NOT NULL DEFAULT FALSE,
  PRIMARY KEY (user_id, date)
);

-- ============ 社区域（M15-M17） ============

CREATE TABLE forums (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  descr TEXT,
  min_class INT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE topics (
  id BIGSERIAL PRIMARY KEY,
  forum_id BIGINT NOT NULL REFERENCES forums(id) ON DELETE CASCADE,
  user_id BIGINT REFERENCES users(id),
  title TEXT NOT NULL,
  sticky BOOLEAN NOT NULL DEFAULT FALSE,
  locked BOOLEAN NOT NULL DEFAULT FALSE,
  views INT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_post_at TIMESTAMPTZ
);

CREATE SEQUENCE posts_id_seq;
CREATE TABLE posts (
  id BIGINT NOT NULL,
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  user_id BIGINT REFERENCES users(id),
  body TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  edited_at TIMESTAMPTZ,
  PRIMARY KEY (topic_id, id, created_at)
) PARTITION BY RANGE (created_at);
CREATE TABLE posts_default PARTITION OF posts DEFAULT;

CREATE TABLE messages (
  id BIGSERIAL PRIMARY KEY,
  sender_id BIGINT REFERENCES users(id),
  receiver_id BIGINT NOT NULL REFERENCES users(id),
  subject TEXT NOT NULL,
  body TEXT NOT NULL,
  read_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE friendships (
  user_id BIGINT NOT NULL REFERENCES users(id),
  friend_id BIGINT NOT NULL REFERENCES users(id),
  list TEXT NOT NULL DEFAULT 'friend',   -- friend/black/pending
  PRIMARY KEY (user_id, friend_id)
);

CREATE TABLE requests (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  title TEXT NOT NULL,
  descr TEXT,
  bounty BIGINT NOT NULL DEFAULT 0,
  fulfilled_torrent_id BIGINT REFERENCES torrents(id),
  status SMALLINT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE offers (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT NOT NULL REFERENCES torrents(id),
  votes INT NOT NULL DEFAULT 0,
  promoted BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE subtitles (
  id BIGSERIAL PRIMARY KEY,
  torrent_id BIGINT REFERENCES torrents(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  title TEXT NOT NULL,
  lang TEXT,
  file_ref TEXT NOT NULL,
  downloads INT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============ 保种 / 考核 / 任务 / 勋章（M19-M22, M14） ============

CREATE TABLE seed_preserve (
  torrent_id BIGINT PRIMARY KEY REFERENCES torrents(id) ON DELETE CASCADE,
  claimed_by BIGINT REFERENCES users(id),
  claimed_at TIMESTAMPTZ,
  exited_at TIMESTAMPTZ,
  exit_reason TEXT                            -- seeders_gt_7 / manual
);

CREATE TABLE jixiao_types (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  metrics JSONB NOT NULL DEFAULT '{}',
  base_pay BIGINT NOT NULL DEFAULT 0,
  min_requirements JSONB NOT NULL DEFAULT '{}',
  bonus_rules JSONB NOT NULL DEFAULT '{}'
);

CREATE TABLE jixiao_claims (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  type_id BIGINT NOT NULL REFERENCES jixiao_types(id),
  period CHAR(7) NOT NULL,
  amount BIGINT NOT NULL,
  metrics_snapshot JSONB,
  claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (user_id, type_id, period)
);

CREATE TABLE tasks (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  metric JSONB NOT NULL DEFAULT '{}',
  starts_at TIMESTAMPTZ NOT NULL,
  ends_at TIMESTAMPTZ NOT NULL,
  target_class INT NOT NULL DEFAULT 0,
  reward BIGINT NOT NULL DEFAULT 0,
  penalty BIGINT NOT NULL DEFAULT 0,
  claim_limit INT
);

CREATE TABLE task_claims (
  id BIGSERIAL PRIMARY KEY,
  task_id BIGINT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  status SMALLINT NOT NULL DEFAULT 0,
  settled_at TIMESTAMPTZ,
  UNIQUE (task_id, user_id)
);

CREATE TABLE medals (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  price BIGINT,
  rarity TEXT,
  limited BOOLEAN NOT NULL DEFAULT FALSE,
  asset_ref TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE user_medals (
  user_id BIGINT NOT NULL REFERENCES users(id),
  medal_id BIGINT NOT NULL REFERENCES medals(id) ON DELETE CASCADE,
  source TEXT NOT NULL DEFAULT 'buy',
  wearing BOOLEAN NOT NULL DEFAULT FALSE,
  PRIMARY KEY (user_id, medal_id)
);

-- ============ 管理与审计（M10 / §5.7） ============

CREATE TABLE reports (
  id BIGSERIAL PRIMARY KEY,
  reporter_id BIGINT NOT NULL REFERENCES users(id),
  ref_type TEXT NOT NULL,
  ref_id BIGINT NOT NULL,
  reason TEXT NOT NULL,
  status SMALLINT NOT NULL DEFAULT 0,
  handled_by BIGINT REFERENCES users(id),
  handled_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE SEQUENCE audit_log_id_seq;
CREATE TABLE audit_log (
  id BIGINT NOT NULL,
  actor_id BIGINT REFERENCES users(id),
  action TEXT NOT NULL,
  ref JSONB,
  ip INET,
  prev_hash BYTEA,
  self_hash BYTEA,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (id)
);
