-- 0374 全域事件 outbox（PT Studio 对标的架构差，本批落 P1：事务写入 + 幂等消费）
--
-- 现状：只有一个局部的 `push_outbox`（0283，worker 产出 → api 投递 Web Push）。
-- 它解决的是「跨 crate 共享投递实现」这一个具体问题，不是事件机制本身：
--   - 没有事务写入（业务提交与事件入队不在同一事务，业务失败事件还在跑，
--     或业务成功事件丢在进程崩溃的缝里）；
--   - 没有失败重试与死信（processed_ok=false 就永远躺在表里，无人再看）；
--   - 没有消费者注册（不知道谁该处理什么）；
--   - 不能跨域（发种事件要通知论坛、计数、插件，全靠每个调用点手写）。
--
-- 本迁移落三件：事件表 + 订阅表 + 幂等/死信口径。
-- 纪律：
--   * 全部幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING）；
--   * 幂等键是**部分唯一**（同键只约束未消费行）——已消费行不占键，
--     否则「同一事件第二次发生」会被历史行永久挡住；
--   * 事件名与订阅名用自由文本 + 注册表对齐，不在 DB 层做枚举约束
--     （枚举进代码，DB 只做存储与形状）。
--
-- ① 事件表：业务事务内写入，与业务写同生共死。
CREATE TABLE IF NOT EXISTS domain_events (
    id          BIGSERIAL PRIMARY KEY,
    event       TEXT        NOT NULL,           -- 事件名，如 torrent.uploaded
    aggregate   TEXT,                          -- 聚合类型，如 torrent / user
    aggregate_id BIGINT,                       -- 聚合 id（可为 NULL：纯广播事件）
    payload     JSONB       NOT NULL DEFAULT '{}'::jsonb,
    dedupe_key  TEXT,                          -- 幂等键（可选）
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    consumed_at TIMESTAMPTZ,                   -- NULL = 未消费
    attempts    INTEGER      NOT NULL DEFAULT 0,
    last_error  TEXT                          -- 最近一次失败原因（死信判据）
);

-- 未消费行的部分索引：消费循环按 id 顺序拉取，这是热路径。
CREATE INDEX IF NOT EXISTS idx_domain_events_pending
    ON domain_events (id) WHERE consumed_at IS NULL;

-- 幂等：同键未消费过才允许入队。已消费行不参与（见纪律第二条）。
CREATE UNIQUE INDEX IF NOT EXISTS idx_domain_events_dedupe
    ON domain_events (dedupe_key)
    WHERE dedupe_key IS NOT NULL AND consumed_at IS NULL;

-- 死信扫查：重试超限的行按 event 聚类，先能回答「哪类事件在失败」。
CREATE INDEX IF NOT EXISTS idx_domain_events_deadletter
    ON domain_events (event, attempts DESC) WHERE consumed_at IS NULL;

-- ② 订阅表：事件名 → 消费方。消费循环按订阅表 fan-out，
--    新增消费方不改业务代码，只加一行（这是 PT Studio 事件总线的核心收益）。
--    active=no 的订阅保留行（站长临时停用某消费方，不必删历史）。
CREATE TABLE IF NOT EXISTS event_subscriptions (
    id         BIGSERIAL PRIMARY KEY,
    event      TEXT        NOT NULL,
    consumer   TEXT        NOT NULL,           -- 消费方标识，如 push / forum / plugin
    active     BOOLEAN     NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ  NOT NULL DEFAULT now(),
    UNIQUE (event, consumer)
);

-- ③ 种子订阅：把 0283 的 push_outbox 消费登记进总线（**不搬数据**——
--    0283 那条链路继续按原样跑，总线订阅先只作登记，避免一次性改动
--    动到已在跑的 Web Push 投递）。其余消费方在各自接线时登记。
INSERT INTO event_subscriptions (event, consumer) VALUES
    ('torrent.uploaded',     'plugin.auto_pin_official'),
    ('user.login',           'plugin'),
    ('torrent.uploaded',     'push')
ON CONFLICT (event, consumer) DO NOTHING;

COMMENT ON TABLE domain_events IS
    '全域事件 outbox：业务事务内写入，消费循环按订阅表 fan-out。幂等键只约束未消费行；attempts 超限且 consumed_at 仍 NULL 的行为死信（last_error 记原因）。';
COMMENT ON COLUMN domain_events.dedupe_key IS
    '幂等键。部分唯一索引只覆盖未消费行——已消费行不占键，否则同一事件的第二次发生会被历史行永久挡住。';