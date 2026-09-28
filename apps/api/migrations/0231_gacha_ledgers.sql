-- 0231_gacha_ledgers.sql — G31-B：抽取券与碎片两套独立账本（方案《抽卡玩法落地方案-2026-09-27》§3 0230–0231）
--
-- 设计约束：与 spark 分轨（spark 是劳动凭证，随机发放带真实价值的东西 = 印钞口；
-- 券不可购买、不可转让，第一批只有发放与抽取两个出口）。账本形状照 spark_ledger
-- 先例：行锁余额 + idempotency_key 幂等 + balance_after 逐条可核 + 同事务写流水。

-- 1) 抽取券（gacha_ticket）：余额行 + 流水
CREATE TABLE gacha_ticket_balance (
    user_id bigint PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance int NOT NULL DEFAULT 0 CHECK (balance >= 0)
);

CREATE TABLE gacha_ticket_ledger (
    id          bigserial PRIMARY KEY,
    user_id     bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    delta       int NOT NULL,
    kind        text NOT NULL,               -- grant / draw / daily_free / refund
    ref_type    text,
    ref_id      bigint,
    idempotency_key text UNIQUE,
    balance_after int NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX gacha_ticket_ledger_user_idx ON gacha_ticket_ledger (user_id, id);

-- 2) 碎片（gacha_shard）：抽取产出、合成/升级消耗（G31-C），同一套形状
CREATE TABLE gacha_shard_balance (
    user_id bigint PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance int NOT NULL DEFAULT 0 CHECK (balance >= 0)
);

CREATE TABLE gacha_shard_ledger (
    id          bigserial PRIMARY KEY,
    user_id     bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    delta       int NOT NULL,
    kind        text NOT NULL,               -- draw / synth / levelup / dismantle
    ref_type    text,
    ref_id      bigint,
    idempotency_key text UNIQUE,
    balance_after int NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX gacha_shard_ledger_user_idx ON gacha_shard_ledger (user_id, id);
