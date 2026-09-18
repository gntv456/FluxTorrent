-- 0125 论坛 Phase2（第二片）：投票主题（poll）
--
-- 范式参照既有 fun_polls（0016，趣味盒投票）：选项存 JSONB、一人一票靠 UNIQUE 约束、
-- ON CONFLICT DO NOTHING 幂等。差异：论坛投票挂在主题上（topic_type='poll'，0115 已就位）、
-- 选项需要稳定的 id 供结果条排序与投票锚定（fun_polls 的数组下标够用，这里沿用数组下标即可——
-- 选项在发帖时定死、不可增删，下标即稳定键）。
--
-- 刻意不做 multi（多选）列：单选覆盖绝大多数论坛场景，多选留到有真实需求时再加列，
-- 避免提前设计（YAGNI）。投票免费（趣味盒扣 1 魔力是游戏口径，论坛投票是表达渠道，不收费）。

CREATE TABLE IF NOT EXISTS topic_polls (
  topic_id BIGINT PRIMARY KEY REFERENCES topics(id) ON DELETE CASCADE,
  -- 选项文本数组：["选项A","选项B",...]，发帖时定死（2~10 项）
  options JSONB NOT NULL,
  -- 楼主可提前截止；截止后只读结果
  closed BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS poll_votes (
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  -- 选项下标（与 topic_polls.options 数组对齐）
  option_index INT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (topic_id, user_id)   -- 一人一票（与 fun_votes 同款约束）
);

CREATE INDEX IF NOT EXISTS idx_poll_votes_topic ON poll_votes (topic_id);
