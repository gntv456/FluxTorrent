-- 0094 P1-1：名额字段统一（claim_limit）。
-- 背景：0027 给 tasks 加了 quota_total（前端 task-board 在读），但 claim/settle 一直只认
-- 0001 的 claim_limit —— 两个字段语义相同却互不同步，运营改哪个都不对。
-- 处理：quota_total 数据并入 claim_limit（仅对 claim_limit IS NULL 的行回填），
--       前端 /tasks 列表改下发 claim_limit；quota_total 列保留但不再写入（SQLite 无 DROP COLUMN
--       约束的历史包袱不存在于 PG，但 sqlx 离线校验与旧回滚兼容起见保留列、标记废弃）。

-- 回填：claim_limit 为 NULL 且 quota_total > 0 的行取 quota_total
UPDATE tasks
   SET claim_limit = quota_total
 WHERE claim_limit IS NULL
   AND COALESCE(quota_total, 0) > 0;

COMMENT ON COLUMN tasks.quota_total IS '废弃（0094）：名额统一走 claim_limit，本列仅历史数据保留，不再读写';
