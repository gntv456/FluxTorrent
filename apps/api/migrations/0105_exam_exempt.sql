-- 0105：考核豁免 / 恢复（对标 NP exam-users 的 avoid / recover / bulkAvoid）。
--
-- 设计：**不动 task_claims.status 枚举（0 进行中 / 1 完成 / 2 失败）**，另加独立标记列。
-- 豁免语义是「暂不参与考核结算，恢复后原记录继续按增量口径结算」：
-- 若用 status=3 会打断 task_settle 只扫 status=0 的状态机，恢复时还得再迁移回 0。
-- 标记列方案下，结算与展示只需排除 exempted_at IS NOT NULL。

ALTER TABLE task_claims ADD COLUMN IF NOT EXISTS exempted_at TIMESTAMPTZ;
ALTER TABLE task_claims ADD COLUMN IF NOT EXISTS exempted_by BIGINT REFERENCES users(id) ON DELETE SET NULL;

-- 结算扫描高频使用：只索引「进行中且未豁免」的行
CREATE INDEX IF NOT EXISTS idx_task_claims_open
    ON task_claims (status) WHERE status = 0 AND exempted_at IS NULL;
