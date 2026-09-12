-- 论坛权限模型（M50，好学 forums.php 三层判定口径）：
-- 1) 版块三档门槛：minclassread 看 / minclasswrite 回 / minclasscreate 发主题（逐版块独立配置）
-- 2) 版主：forum_mods（forumid + userid，无需等级，仅在本版块有效）
-- 3) 全局：postmanage（≥90 全版块管帖）/ forummanage（≥93 建版块/设门槛/任免版主）
-- 另：受保护版块（2楼起正文替换为提示）、账户级禁言 forumpost、帖子编辑留痕 edited_by
ALTER TABLE forums
  ADD COLUMN IF NOT EXISTS minclassread INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS minclasswrite INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS minclasscreate INT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS protected BOOLEAN NOT NULL DEFAULT FALSE;

-- 旧 min_class 平移为三档初始值
UPDATE forums SET minclassread = min_class, minclasswrite = min_class, minclasscreate = min_class
WHERE minclassread = 0 AND minclasswrite = 0 AND minclasscreate = 0 AND min_class > 0;

CREATE TABLE IF NOT EXISTS forum_mods (
  forum_id BIGINT NOT NULL REFERENCES forums(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_by BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (forum_id, user_id)
);

-- 账户级禁言开关（forumpost='no' 口径）：true 可发帖回帖
ALTER TABLE users ADD COLUMN IF NOT EXISTS forumpost BOOLEAN NOT NULL DEFAULT TRUE;

-- 发帖 10 秒防刷（postmanage 豁免）
CREATE TABLE IF NOT EXISTS forum_flood (
  user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
  last_sent_at TIMESTAMPTZ NOT NULL
);

-- 帖子编辑留痕（管理员编辑他人帖自动 PM 通知作者）
ALTER TABLE posts ADD COLUMN IF NOT EXISTS edited_by BIGINT REFERENCES users(id);

-- 种子页/forum 列表筛选：三档门槛索引
CREATE INDEX IF NOT EXISTS idx_forums_minclassread ON forums (minclassread);
