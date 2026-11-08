-- 0481 考核收回（2026-11-08）：管理员可撤下错发/误派的考核记录
--
-- 背景：迁移期 users.created_at 曾短暂等于迁移时刻，worker 按其 30 天
-- 新人窗把老号扫进了「新人转正考核」（4 条，其中 2 条次日秒过领走 2000×2）。
-- 派发 SQL 本身无误，但缺收回能力（豁免只是免结算，记录仍挂在会员面板）。
--
-- 口径（0105 exempted_at 同款标记列，不动 status 枚举）：
--   canceled_at 非空 = 记录被撤下：结算扫描跳过、会员面板隐藏、
--   行保留（exam_assign 的 NOT EXISTS 以行存在为界——删行会让窗口内
--   用户下个 tick 被重新派发）。已发奖励由 revoke 端点走
--   clawback_spark_tx 追回（地板 0 + 幂等键 exam_revoke:{claim_id}）。
ALTER TABLE task_claims ADD COLUMN canceled_at timestamptz;
ALTER TABLE task_claims ADD COLUMN canceled_by bigint;
