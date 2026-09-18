-- 0121 论坛 Phase1（第二片）：关注订阅（用户 / 版块 / 主题）
--
-- 为什么不复用 friendships？friendships.list 走的是 pending|friend|black 的**双向好友申请**流程
-- （community_http.rs 有 accept/reject/blacklist），而「关注」是单向、无需对方同意的订阅语义，
-- 塞进 friendships 会把两种语义搅在一起。故独立建表。
--
-- target_id 指向 users / forums / topics 三张不同表，无法建外键，因此：
--   · user/forum 目标删除由应用层清理（admin 删用户已有清理范式的先例）；
--   · topic 目标删除在 topic_delete 里显式 DELETE（与 post_likes 同一处置）。
-- PK(user_id,target_type,target_id) 天然幂等，防重复关注。

CREATE TABLE IF NOT EXISTS follows (
  user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  target_type TEXT   NOT NULL,
  target_id   BIGINT NOT NULL,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, target_type, target_id),
  CONSTRAINT follows_target_type_chk CHECK (target_type IN ('user', 'forum', 'topic'))
);

-- 反查「谁关注了这个对象」：发新主题/回帖时要按 (target_type, target_id) 捞关注者
CREATE INDEX IF NOT EXISTS idx_follows_target ON follows (target_type, target_id);
-- 「我的关注列表」按 (user_id, target_type) 捞
CREATE INDEX IF NOT EXISTS idx_follows_user ON follows (user_id, target_type);
