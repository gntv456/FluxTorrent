-- 0154 评论点赞（六维强化方案 阶段三「评论增强」第一步）
--
-- 口径：
--  * 一人一评一赞（PK 即幂等约束，toggle 端点 INSERT ... ON CONFLICT DO NOTHING / DELETE）；
--  * 不允许给自己的评论点赞（端点校验，这里不加约束——好友互赞是合法场景，
--    「自赞」拦在业务层，错误信息更友好）；
--  * 计数不落表（list_comments 实时 COUNT，评论页深翻页前无热点）。

CREATE TABLE IF NOT EXISTS comment_likes (
  comment_id BIGINT NOT NULL REFERENCES comments(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (comment_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_comment_likes_user ON comment_likes (user_id);
