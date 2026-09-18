-- 0124 论坛 Phase2（第一片）：悬赏主题（bounty）接 spark 经济
--
-- 设计对齐既有「求种悬赏」（content_http.rs requests.bounty）的冻结/发放范式：
--   发帖冻结（spend_spark_tx 与建主题同事务）→ 楼主采纳某回复 → 发放给答主（earn_spark 幂等）。
-- 策划案原案的 topic_bounty_claims（多人认领）从简：悬赏语义是「楼主单方面采纳一个最佳答案」，
-- 不需要认领表——采纳行为本身只发生一次（bounty_status 从 open 迁走即 CAS 防重）。
--
-- topics 直接加列（不加子表）：详情页一次取齐，且 topic_type='bounty' 已在 0115 落位。

ALTER TABLE topics
  ADD COLUMN IF NOT EXISTS bounty_spark BIGINT NOT NULL DEFAULT 0,
  -- open=悬赏中 / awarded=已采纳发放 / refunded=已退回（删主题/主动取消）
  ADD COLUMN IF NOT EXISTS bounty_status TEXT NOT NULL DEFAULT 'open',
  -- 采纳的回复 post_id（审计留痕：谁拿了钱、因哪一楼）
  ADD COLUMN IF NOT EXISTS bounty_post_id BIGINT;

-- 采纳记录独立成表而非只靠 topics 列：楼主可改采（撤回旧采纳再采新的），
-- 每次采纳/撤回留一行审计流水，钱的进出则以 spark_ledger 幂等键为准。
CREATE TABLE IF NOT EXISTS topic_bounty_awards (
  id BIGSERIAL PRIMARY KEY,
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  post_id BIGINT NOT NULL,
  answerer_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  awarded_by BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  spark BIGINT NOT NULL,
  undone BOOLEAN NOT NULL DEFAULT FALSE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_bounty_awards_topic ON topic_bounty_awards (topic_id);
