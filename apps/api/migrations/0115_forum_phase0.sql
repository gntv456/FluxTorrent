-- 0115 论坛 Phase0（信息架构与内容基座，见 _doc/论坛版块完善策划案.md）：
-- 1) 分区/节点 forum_categories，forums.category_id 归属（首页按分类分组）
-- 2) topics.topic_type 帖子类型（normal|bounty|poll|lottery；本迁移仅落列与默认值，形态逻辑后续迁移）
-- 3) posts.body_text 正文纯文本摘要（供搜索/列表预览；前端渲染 Markdown 时搜索仍可用）
-- 全部 IF NOT EXISTS，幂等；回填 + 默认分区保证首页分组不空。

CREATE TABLE IF NOT EXISTS forum_categories (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  sort INT NOT NULL DEFAULT 0,
  visible BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE forums ADD COLUMN IF NOT EXISTS category_id BIGINT REFERENCES forum_categories(id) ON DELETE SET NULL;
ALTER TABLE topics ADD COLUMN IF NOT EXISTS topic_type TEXT NOT NULL DEFAULT 'normal';
ALTER TABLE posts  ADD COLUMN IF NOT EXISTS body_text TEXT;

CREATE INDEX IF NOT EXISTS idx_forums_category ON forums (category_id);
CREATE INDEX IF NOT EXISTS idx_topics_type ON topics (topic_type);

-- 既有帖子回填纯文本摘要（历史 body 本就是纯文本，直接平移）
UPDATE posts SET body_text = body WHERE body_text IS NULL;

-- 默认分区：保证首页有分组可挂；把尚无分类的版块挂到首个分类
INSERT INTO forum_categories (name, sort)
SELECT '综合交流', 0
WHERE NOT EXISTS (SELECT 1 FROM forum_categories);

UPDATE forums
SET category_id = (SELECT id FROM forum_categories ORDER BY sort, id LIMIT 1)
WHERE category_id IS NULL
  AND EXISTS (SELECT 1 FROM forum_categories);
