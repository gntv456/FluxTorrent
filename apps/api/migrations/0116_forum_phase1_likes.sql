-- 0116 论坛 Phase1（互动与成长）：点赞 + 收藏
-- post_likes：对某一楼帖子点赞（posts 是分区表，无法对 post_id 建 FK，故按 topic_id 级联清理；
--   post_delete 单帖删除时由应用层清理）。PK(user_id, post_id) 天然幂等，防重复点赞。
-- topic_favorites：收藏主题（外键 topics ON DELETE CASCADE，删主题自动清理）。

CREATE TABLE IF NOT EXISTS post_likes (
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  post_id BIGINT NOT NULL,
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, post_id)
);
CREATE INDEX IF NOT EXISTS idx_post_likes_post ON post_likes (post_id);
CREATE INDEX IF NOT EXISTS idx_post_likes_topic ON post_likes (topic_id);
CREATE INDEX IF NOT EXISTS idx_post_likes_user ON post_likes (user_id);

CREATE TABLE IF NOT EXISTS topic_favorites (
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, topic_id)
);
CREATE INDEX IF NOT EXISTS idx_topic_fav_user ON topic_favorites (user_id);
CREATE INDEX IF NOT EXISTS idx_topic_fav_topic ON topic_favorites (topic_id);
