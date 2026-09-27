-- 0220：修复 root 起始余额被测试扣费扣成负数（数据修复，非产品缺陷）
--
-- 背景：root（id=1）在 0017 初始化时 spark_balance=100000，但该值**未写 spark_ledger**。
--   此后 2026-09-25 的端到端 / 回归测试（kind='shop'，idempotency_key 前缀
--   'e2e-0204-' / 'medal-gift:' / 'shop:1:regr-buy-'）连续扣费共约 -62,002，
--   而账面正规入账仅 +12（forum +2 / attendance +10），致 root 余额落到 -61990，
--   商店所有商品显示「余额不足」，新站长第一眼会以为商店坏了。
-- 定性：**这不是产品缺陷**——统一扣费入口 economy_http/spend.rs::spend_spark
--   有 `if balance < amount { InsufficientSpark }` 保护（错误码 4001「余额不足」），
--   正常用户路径不会透支；负值源自「初始余额未入账 + 测试大量消费」的历史残留。
-- 修复（幂等、保守）：
--   仅当 root 余额 < 0 时，补一笔 initial_grant 入账，把余额抬回 0，使
--   spark_ledger 汇总与 users.spark_balance 重新对齐（账平）。
--   正常库（root 非负）不受影响；不抬高正常余额，不删除任何既有流水。
-- 说明：0017 直接写 balance 不写 ledger 是「初始赠予不入账」的设计，本迁移
--   顺带为 root 补记该语义的流水，避免后续对账（reconcile）持续报差异。

WITH deficit AS (
    SELECT u.id AS user_id, -u.spark_balance AS need
    FROM users u
    WHERE u.id = 1 AND u.spark_balance < 0
),
ins AS (
    INSERT INTO spark_ledger
        (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after)
    SELECT
        nextval('spark_ledger_id_seq'),
        d.user_id,
        d.need,
        'initial_grant',
        'bootstrap',
        0,
        'bootstrap-fix-root-negative-' || d.user_id,
        0
    FROM deficit d
    ON CONFLICT DO NOTHING
    RETURNING user_id
)
UPDATE users u
SET spark_balance = 0
FROM deficit d
WHERE u.id = d.user_id;
