-- 0030: staffpanel 管理工具落地（FAQ 管理/规则管理/分类管理/封禁系统/批量邮件）
-- + 插件移植表（勋章墙 contest 大赛 / 头像挂件 / 五子棋对局）

-- FAQ 管理（faqmanage.php 口径：分类 + 顺序 + 富文本）
CREATE TABLE IF NOT EXISTS faq_items (
  id SERIAL PRIMARY KEY,
  lang TEXT NOT NULL DEFAULT 'chs',
  category TEXT NOT NULL DEFAULT 'default',
  question TEXT NOT NULL,
  answer TEXT NOT NULL,
  sort INT NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 规则管理（modrules.php 口径：多条规则段）
CREATE TABLE IF NOT EXISTS site_rules (
  id SERIAL PRIMARY KEY,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  sort INT NOT NULL DEFAULT 0,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 封禁系统（bans.php 口径：IP 封禁）
CREATE TABLE IF NOT EXISTS ip_bans (
  id SERIAL PRIMARY KEY,
  ip INET NOT NULL UNIQUE,
  reason TEXT,
  banned_by BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 批量邮件记录（massmail.php 口径：发送历史）
CREATE TABLE IF NOT EXISTS mass_mails (
  id SERIAL PRIMARY KEY,
  subject TEXT NOT NULL,
  body TEXT NOT NULL,
  sent_by BIGINT REFERENCES users(id),
  recipients INT NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 大赛（contest.php 口径：用户报名 + 积分排行）
CREATE TABLE IF NOT EXISTS contests (
  id SERIAL PRIMARY KEY,
  title TEXT NOT NULL,
  descr TEXT,
  starts_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  ends_at TIMESTAMPTZ NOT NULL DEFAULT now() + interval '30 days',
  is_active BOOLEAN NOT NULL DEFAULT TRUE
);
CREATE TABLE IF NOT EXISTS contest_entries (
  id SERIAL PRIMARY KEY,
  contest_id INT NOT NULL REFERENCES contests(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  score INT NOT NULL DEFAULT 0,
  UNIQUE (contest_id, user_id)
);

-- 头像挂件（avatar_frame.php 口径：用户头像装饰框）
CREATE TABLE IF NOT EXISTS avatar_frames (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  css TEXT NOT NULL,               -- 生效的 CSS（边框/光环效果）
  price INT NOT NULL DEFAULT 0,    -- 魔力价格
  sort INT NOT NULL DEFAULT 0
);
ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_frame_id INT REFERENCES avatar_frames(id);

-- 五子棋对局（wuziqi.php 口径：15x15，两人对局）
CREATE TABLE IF NOT EXISTS gomoku_games (
  id SERIAL PRIMARY KEY,
  black_id BIGINT NOT NULL REFERENCES users(id),
  white_id BIGINT REFERENCES users(id),
  board TEXT NOT NULL DEFAULT '',  -- 225 格，'b'/'w'/空
  turn CHAR(1) NOT NULL DEFAULT 'b',
  winner_id BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 幂等种子
INSERT INTO faq_items (category, question, answer, sort)
SELECT * FROM (VALUES
  ('default', '什么是分享率？', '分享率 = 上传量 ÷ 下载量。站点要求保持分享率在 1.0 以上。', 1),
  ('default', '如何邀请好友？', '达到一定等级后可获得邀请名额，在「邀请」页面生成邀请码发送给好友。', 2),
  ('default', '做种积分怎么计算？', '按做种数量与体积综合计算，详见任务系统说明。', 3)
) AS seed(category, question, answer, sort)
WHERE NOT EXISTS (SELECT 1 FROM faq_items);

INSERT INTO site_rules (title, body, sort)
SELECT * FROM (VALUES
  ('通用规则', '1. 禁止发布任何违反法律法规的内容；2. 禁止交易账号；3. 保持友善交流。', 1),
  ('上传规则', '1. 保证资源真实有效；2. 命名规范；3. 做种至少 72 小时或有人接续。', 2),
  ('H&R 规则', '完成下载的种子需在 14 天内做种满 120 小时，否则计入 H&R。', 3)
) AS seed(title, body, sort)
WHERE NOT EXISTS (SELECT 1 FROM site_rules);

INSERT INTO avatar_frames (name, css, price, sort)
SELECT * FROM (VALUES
  ('金色光环', 'box-shadow: 0 0 0 3px #d4a017, 0 0 12px rgba(212,160,23,.6);', 5000, 1),
  ('蓝色流光', 'box-shadow: 0 0 0 3px #2f7bd9, 0 0 12px rgba(47,123,217,.6);', 5000, 2),
  ('彩虹描边', 'box-shadow: 0 0 0 3px transparent; border: 3px solid; border-image: linear-gradient(45deg,#eb510e,#d99419,#2e7d43,#2f7bd9) 1;', 10000, 3)
) AS seed(name, css, price, sort)
WHERE NOT EXISTS (SELECT 1 FROM avatar_frames);

INSERT INTO contests (title, descr, is_active)
SELECT '首届做种大赛', '活动期间做种积分排行前 10 名可获得限定勋章。', TRUE
WHERE NOT EXISTS (SELECT 1 FROM contests);
