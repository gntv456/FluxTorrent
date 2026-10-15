-- 0379 抽奖可验证公平落库（commit-reveal，PT Studio FR-LOT 对标）
--
-- 纯函数层在 api `domain/lottery_fair.rs`（SHA-256 / HMAC / 排序）。
-- 本迁移只提供**落点**：承诺、票据链、结算证明三组字段。
--
-- 为什么加在 topic_lotteries 而不是新表：
--   一场抽奖就是 topic_lotteries 的一行（topic_id 主键），另起表会多一次
--   JOIN 且两处状态可能漂移。票据链同理挂在 lottery_entries 上。
--
-- 兼容口径：新列全部可空，旧抽奖（已 drawn / 无 commit）照常工作——
--   worker 结算时 commit 为空则走旧路径（ORDER BY random()），
--   新开的抽奖必定带 commit。已发出的证明永不撤回。

-- ① 承诺：开售时写 commit（公布），seed_cipher 只在结算时读出。
ALTER TABLE topic_lotteries
    ADD COLUMN IF NOT EXISTS commit_hash TEXT,
    ADD COLUMN IF NOT EXISTS seed_cipher TEXT,
    ADD COLUMN IF NOT EXISTS tickets_root TEXT,
    ADD COLUMN IF NOT EXISTS draw_rand TEXT,
    ADD COLUMN IF NOT EXISTS draw_proof JSONB,
    ADD COLUMN IF NOT EXISTS fair_enabled BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN topic_lotteries.commit_hash IS
    'commit-reveal 承诺：SHA-256(seed)，开售即公布。NULL = 老抽奖走旧随机路径。';
COMMENT ON COLUMN topic_lotteries.seed_cipher IS
    '32B seed（hex）。揭晓前不得外泄；结算后随证明一并公开。';
COMMENT ON COLUMN topic_lotteries.tickets_root IS
    '票据哈希链末条。停售时锁死；改动名单会让公开的 rand 对不上。';
COMMENT ON COLUMN topic_lotteries.draw_rand IS
    'HMAC-SHA-256(seed, round_id‖tickets_root)，结算后公开供复算。';
COMMENT ON COLUMN topic_lotteries.draw_proof IS
    '完整证明（seed/tickets_root/rand/中奖票据哈希），结算后公开供第三方独立验算。';

-- ② 票据：每个参与者一行，票据哈希串成链。
--    ticket_hash = SHA-256(seed ‖ prev_hash ‖ nonce)，nonce 用全局序保证唯一。
ALTER TABLE lottery_entries
    ADD COLUMN IF NOT EXISTS ticket_hash TEXT,
    ADD COLUMN IF NOT EXISTS ticket_seq BIGINT;

COMMENT ON COLUMN lottery_entries.ticket_hash IS
    '本张票据的哈希（commit-reveal 哈希链一环）。结算后公开供复算排序键。';
COMMENT ON COLUMN lottery_entries.ticket_seq IS
    '票据序号（入链次序）。同场唯一，支撑「按序重建哈希链」验算。';

-- 同场票据序号唯一：链式哈希要求前驱唯一，否则验算无法定位断点。
CREATE UNIQUE INDEX IF NOT EXISTS idx_lottery_entries_seq
    ON lottery_entries (topic_id, ticket_seq)
    WHERE ticket_seq IS NOT NULL;

-- 结算证明按场查询：验证页要能取到完整材料。
CREATE INDEX IF NOT EXISTS idx_lottery_fair_proof
    ON topic_lotteries (topic_id)
    WHERE draw_proof IS NOT NULL;
