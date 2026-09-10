-- 0029: 聊天盒（shoutbox.php 复刻：站内公共聊天）
CREATE TABLE IF NOT EXISTS shoutbox (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  message TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS shoutbox_created ON shoutbox (created_at DESC);
-- H&R 达标口径：120 小时做种。hr_flag 标记未达标但已超过宽限期的记录。
UPDATE snatches SET hr_flag = TRUE
WHERE completed_at IS NOT NULL AND seeded_seconds < 432000
  AND completed_at < now() - interval '14 days';
