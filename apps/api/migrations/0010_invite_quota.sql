-- P2 修复：邀请配额原子占位表（防 count+insert 并发穿透）
CREATE TABLE IF NOT EXISTS invite_quota (
  user_id BIGINT NOT NULL REFERENCES users(id),
  period DATE NOT NULL,
  used INT NOT NULL DEFAULT 0,
  PRIMARY KEY (user_id, period)
);
