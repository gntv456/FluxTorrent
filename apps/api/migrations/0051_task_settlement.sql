-- 0051 任务结算链路（2026-09-12）：领取后有进度基线、达标发奖、超时失败。
-- base + delta 口径（对齐好学站 UserTaskRecord 的 points_base/seeding_base 模式）：
-- 领取时记下当时指标快照，结算时用「当前值 - 基线」比对 metric 目标，避免历史存量误判达标。

ALTER TABLE task_claims
  ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  ADD COLUMN IF NOT EXISTS base_uploaded BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS base_seed_seconds BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS base_uploads BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS reward_paid BIGINT NOT NULL DEFAULT 0;

-- 进行中认领的结算扫描（status=0 + 任务过期/时限到）
CREATE INDEX IF NOT EXISTS idx_task_claims_open ON task_claims (status) WHERE status = 0;
