-- 补建 offer_votes（0001 遗漏）
CREATE TABLE IF NOT EXISTS offer_votes (
  offer_id BIGINT NOT NULL REFERENCES offers(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  cost BIGINT NOT NULL DEFAULT 1,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (offer_id, user_id)
);
