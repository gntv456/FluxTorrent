-- 幂等结果回放（2026-11-21）
--
-- 背景：池抽类玩法（九宫格/扭蛋/大转盘）首局提交成功但响应丢包时，
-- 客户端用同一幂等键重试，旧实现只返回「该局已受理」，拿不回原结果。
-- 这里把「这一局到底抽中了什么」和扣票/结算放在同一事务里落库；
-- 重试命中幂等后直接回放，不再重抽，也不让玩家账面扣款却看不到奖。
--
-- 只新增表，不改 arcade_pool_rounds；历史局没有 idempotency_key，保持原样。

BEGIN;

CREATE TABLE IF NOT EXISTS arcade_draw_results (
    id              bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    idempotency_key text NOT NULL UNIQUE,
    user_id         bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    game            text NOT NULL,
    entry_index     integer NOT NULL,
    prize           text NOT NULL,
    rarity          smallint NOT NULL DEFAULT 1,
    kind            text NOT NULL DEFAULT 'magic',
    fell_back       text,
    replay          boolean NOT NULL DEFAULT false,
    payout          bigint NOT NULL DEFAULT 0,
    value           bigint NOT NULL DEFAULT 0,
    ticket          bigint NOT NULL DEFAULT 0,
    created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_arcade_draw_results_user
    ON arcade_draw_results (user_id, created_at DESC);

COMMENT ON TABLE arcade_draw_results IS
    '池抽类玩法的幂等回放快照：同一事务写入扣票/结算/结果，响应丢包后可重放';

COMMIT;
