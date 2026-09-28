-- 0237：G31 审查批 P0-3 余额基线（0231 gacha/0236 email_verify 均被并行会话占用）
--
-- 审查发现：users.uploaded/downloaded 与 spark 余额的快照不是独立权威，
--   而是每分钟/每 6h 从流水全量重算（SUM(traffic_ledger)）。直接 DROP 窗口外
--   分区会把老用户余额清零——「对账快照已聚合」的旧表述与事实相反。
-- 处理：建 balance_baseline（user_id 主键），归档 DETACH 前 worker 先把
--   「将被移出的分区」聚合进基线；快照重算改为 baseline + SUM(剩余窗口内)。
--   基线行只在归档时写入，retain=0（缺省）时永远为空 → 快照公式退化为
--   原样（0 + SUM 全表）。

CREATE TABLE IF NOT EXISTS balance_baseline (
    user_id       bigint PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    base_up       bigint NOT NULL DEFAULT 0,
    base_down     bigint NOT NULL DEFAULT 0,
    base_spark    bigint NOT NULL DEFAULT 0,
    base_seed_secs bigint NOT NULL DEFAULT 0,
    through       timestamptz NOT NULL,
    updated_at    timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS balance_baseline_through_idx
    ON balance_baseline (through);
