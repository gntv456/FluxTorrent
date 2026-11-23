-- 甜梦钓场（2026-11-21）：站内完整渔场的服务端存档与局记录。
--
-- 与竞猜式钓鱼（arcade_fishing_rounds）分开：这里货币是场内 magic，
-- 不接站点魔力账；服务端只保存存档、掷骰发鱼、按时刻校验起竿。
--
-- 存档是一整份 JSON（与客户端 engine.defaultState() 同形状），
-- 写入一律在 `SELECT … FOR UPDATE` 事务里做，避免多标签页覆盖。

BEGIN;

CREATE TABLE IF NOT EXISTS arcade_fishing_pond (
    user_id     bigint PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    state       jsonb NOT NULL DEFAULT '{}'::jsonb,
    updated_at  timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE arcade_fishing_pond IS
    '甜梦钓场存档：整份 JSON，字段与客户端 engine.defaultState() 一致';

-- 一竿一行：鱼种/体型/咬钩时刻在抛竿时定死，起竿只判窗口，结算只认这一行
CREATE TABLE IF NOT EXISTS arcade_fishing_pond_rounds (
    id            bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id       bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    fish_id       text NOT NULL,
    rarity        text NOT NULL,
    size_cm       numeric(6,2) NOT NULL,
    cast_stats    jsonb NOT NULL,
    bite_after_ms integer NOT NULL,
    window_ms     integer NOT NULL,
    bite_at       timestamptz NOT NULL,
    hooked_at     timestamptz,
    resolved      boolean NOT NULL DEFAULT false,
    result        jsonb,
    created_at    timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT arcade_fishing_pond_rounds_window CHECK (window_ms BETWEEN 200 AND 8000)
);

CREATE INDEX IF NOT EXISTS idx_fishing_pond_rounds_user
    ON arcade_fishing_pond_rounds (user_id, resolved, created_at DESC);

COMMENT ON TABLE arcade_fishing_pond_rounds IS
    '甜梦钓场一竿：鱼种与咬钩时刻由服务端定，客户端只回传溜鱼结果';

COMMIT;
