-- 信箱对齐 NexusPHP messages.php 全功能（2026-09-12）：
-- 逻辑删除（location/saved 双删语义）、自建文件夹、未读独立列、答复工作台、防刷窗口。
ALTER TABLE messages
  ADD COLUMN IF NOT EXISTS location INT NOT NULL DEFAULT 1;     -- 收件方可见性：1=收件箱 0=收件方已删 -1=系统
ALTER TABLE messages
  ADD COLUMN IF NOT EXISTS saved INT NOT NULL DEFAULT 0;          -- 发件方可见性：1=发件箱 0=发件方已删
ALTER TABLE messages
  ADD COLUMN IF NOT EXISTS folder INT;                            -- 收件方自建文件夹（pmboxes.id）
ALTER TABLE messages
  ADD COLUMN IF NOT EXISTS unread BOOLEAN NOT NULL DEFAULT TRUE;  -- 未读（read_at 保留给展示时间）

-- 自建文件夹（editmailboxes：一人最多 3 个，名字 ≤14 字符）
CREATE TABLE IF NOT EXISTS pmboxes (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  name TEXT NOT NULL CHECK (char_length(name) <= 14),
  UNIQUE (user_id, name)
);

-- 管理组咨询工作台（contactstaff → staffbox.php 口径）
CREATE TABLE IF NOT EXISTS staffmessages (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),     -- 来信人
  subject TEXT NOT NULL,
  body TEXT NOT NULL,
  answered INT NOT NULL DEFAULT 0,                   -- 0=未答复 1=已答复
  answered_by BIGINT REFERENCES users(id),           -- 答复人
  answer TEXT,                                       -- 答复原文回写
  answered_at TIMESTAMPTZ,
  permission TEXT,                                   -- 分流标签（torrent-approval 等；空=普通咨询全员可见）
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_staffmessages_answered ON staffmessages (answered, id DESC);

-- 发信防刷窗口（NexusPHP 口径：普通用户 60s 一条；staff 不限）
CREATE TABLE IF NOT EXISTS message_flood (
  user_id BIGINT PRIMARY KEY REFERENCES users(id),
  last_sent_at TIMESTAMPTZ NOT NULL
);
