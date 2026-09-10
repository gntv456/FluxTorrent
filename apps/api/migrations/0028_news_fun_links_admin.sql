-- 0028: 公告/趣味盒/友情链接 管理（复刻 NexusPHP news.php / fun.php / linksmanage.php）
-- 公告补管理字段（发布者 + 排序）
ALTER TABLE announcements ADD COLUMN IF NOT EXISTS author_id BIGINT REFERENCES users(id);
ALTER TABLE announcements ADD COLUMN IF NOT EXISTS sort INT NOT NULL DEFAULT 0;

-- 趣味盒（包子站 fun.php 口径：标题+正文+状态 normal/dull/notfunny/funny/veryfunny/banned）
CREATE TABLE IF NOT EXISTS fun_items (
  id SERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  title VARCHAR(255) NOT NULL DEFAULT '',
  body TEXT,
  status TEXT NOT NULL DEFAULT 'normal'
    CHECK (status IN ('normal','dull','notfunny','funny','veryfunny','banned')),
  added TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS fun_items_status_added ON fun_items (status, added DESC);

-- 趣味盒投票（fun/dull 一人一条目一票）
CREATE TABLE IF NOT EXISTS fun_item_votes (
  fun_id INT NOT NULL REFERENCES fun_items(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  vote TEXT NOT NULL DEFAULT 'fun' CHECK (vote IN ('fun','dull')),
  added TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (fun_id, user_id)
);

-- 友情链接补管理字段（状态: pending 待审核 / active 显示 / hidden 隐藏）
ALTER TABLE friend_links ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'active'
  CHECK (status IN ('pending','active','hidden'));
ALTER TABLE friend_links ADD COLUMN IF NOT EXISTS applied_by BIGINT REFERENCES users(id);
ALTER TABLE friend_links ADD COLUMN IF NOT EXISTS admin_name TEXT;
ALTER TABLE friend_links ADD COLUMN IF NOT EXISTS email TEXT;
ALTER TABLE friend_links ADD COLUMN IF NOT EXISTS reason TEXT;

-- 幂等种子：趣味盒演示两条
INSERT INTO fun_items (user_id, title, body, status, added)
SELECT u.id, v.title, v.body, 'normal', now() - (v.hours || ' hours')::interval
FROM (VALUES
  ('root', '程序员测试段子', '为什么程序员总是分不清万圣节和圣诞节？因为 Oct 31 == Dec 25。', 30),
  ('root', '种子站冷笑话', '问：做种最怕什么？答：孤单（没有 Leecher）。', 6)
) AS v(uname, title, body, hours)
JOIN users u ON u.username = v.uname
WHERE NOT EXISTS (SELECT 1 FROM fun_items);

-- 序列对齐（0026 种子显式指定 id，序列未推进会导致插入冲突）
SELECT setval('announcements_id_seq', (SELECT COALESCE(max(id), 1) FROM announcements));
SELECT setval('friend_links_id_seq', (SELECT COALESCE(max(id), 1) FROM friend_links));
