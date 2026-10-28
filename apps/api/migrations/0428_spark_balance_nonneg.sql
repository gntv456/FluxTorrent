-- 0428 火花余额禁止为负（P-463 的收口件）。
--
-- 为什么现在加：`users.spark_balance` 是 `bigint NOT NULL` 却没有下界
-- （`admin_http/clawback.rs:4` 自己写着这个洞）。现网实测出现过 39 个负余额号：
-- 27 个 status=2、12 个 status=3，全部是闸门探针/墓碑号，合计 -446731——
-- 成因是「直接写快照而不落流水、也不抬 baseline」，被 `jobs/seeding.rs` 的
-- 每小时重算按 `baseline + SUM(ledger)` 抹平后落到负数。清零与备份已在
-- 2026-10-28 由 psql 事务完成（备份表 `_bak_spark_neg_20261028`，39 行）。
--
-- 为什么幂等：sqlx 只按版本号记账，本文件对**已手工加过约束**的库必须是无操作，
-- 对新站则要真的把约束建起来；`VALIDATE` 对已生效约束是空操作。
-- 用 NOT VALID + VALIDATE 两步：加锁轻，存量行校验一次完成。
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'users_spark_balance_nonneg'
    ) THEN
        ALTER TABLE users
            ADD CONSTRAINT users_spark_balance_nonneg
            CHECK (spark_balance >= 0) NOT VALID;
    END IF;
    ALTER TABLE users VALIDATE CONSTRAINT users_spark_balance_nonneg;
END $$;
