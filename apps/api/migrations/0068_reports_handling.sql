-- 举报状态机补 handling 中间态（社区板块审计 P2-2）：
-- 原两态（0 pending → 1 handled）缺「处理中」，多位管理员同时看队列无法认领防重复处理。
-- 2 = handling（已认领处理中）；resolve 端点接受 0/2 → 1，并支持 0→2 认领。
-- 兼容：status=0/1 语义不变，旧数据无需迁移。

ALTER TABLE reports
  ADD COLUMN IF NOT EXISTS claimed_by BIGINT REFERENCES users(id),
  ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_reports_status ON reports (status, created_at);
