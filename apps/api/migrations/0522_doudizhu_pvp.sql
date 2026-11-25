-- 0522 斗地主 PvP 房间 + 癞子模式（2026-11-25）。
--
-- PvP：mode='pvp' 的对局走房间制——房间等三真人、房主开局后按座位
-- 建 arcade_doudizhu_games 行（每人一行，user_id 各异，复用全部既有
-- 结算/战绩/抽水链路）。房间表只管「撮合与座位」，牌局状态仍在
-- 每人一行的 games 表（seat 映射进 state jsonb 已有 seat 语义）。
--
-- 癞子模式：mode 复用列加 'wild' 变体？不——癞子是规则变体不是经济
-- 模式，落在房间/局表的 variant 列（'classic' | 'wild'），AI 房与
-- PvP 房都能开癞子桌。
--
-- 座位占用：room_seats 一行一座（3 座），部分唯一索引保证一座一人；
-- 匹配 = 找 waiting 房间的空座插入，满员自动开打。

BEGIN;

-- ── 1) 房间表：一行一桌 ──────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS arcade_doudizhu_rooms (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    variant      text NOT NULL DEFAULT 'classic'
                 CHECK (variant IN ('classic', 'wild')),
    stake        bigint NOT NULL CHECK (stake > 0),
    status       text NOT NULL DEFAULT 'waiting'
                 CHECK (status IN ('waiting', 'playing', 'settled',
                                   'abandoned')),
    owner_id     bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    game_id      bigint,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE arcade_doudizhu_rooms IS
    '斗地主 PvP 房间：waiting 等三人 → playing 开打（game_id 指向公共局组）→ settled；abandoned = 房主散桌';

-- 房间座位：一座一行；同一房间内一人一座（部分唯一索引兜底并发抢座）。
-- active = 房间尚在 waiting/playing（进房置 true，散桌/结算统一置 false），
-- 「一人同时只在一只活跃房」用它的部分唯一索引表达（PG 索引谓词禁子查询）。
CREATE TABLE IF NOT EXISTS arcade_doudizhu_room_seats (
    room_id     bigint NOT NULL
                REFERENCES arcade_doudizhu_rooms (id) ON DELETE CASCADE,
    seat        smallint NOT NULL CHECK (seat IN (0, 1, 2)),
    user_id     bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    ready       boolean NOT NULL DEFAULT false,
    active      boolean NOT NULL DEFAULT true,
    joined_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (room_id, seat)
);

CREATE INDEX IF NOT EXISTS idx_dz_seats_user
    ON arcade_doudizhu_room_seats (user_id);

COMMENT ON TABLE arcade_doudizhu_room_seats IS
    '斗地主 PvP 座位：room_id+seat 主键；active=房间存活中；一人一活跃房靠部分唯一索引';

-- 同一房间不重复坐（防并发抢两座）
CREATE UNIQUE INDEX IF NOT EXISTS uq_dz_seats_user_per_room
    ON arcade_doudizhu_room_seats (room_id, user_id);

-- 一个人同时只在一只 waiting/playing 房间里（全局排队纪律）
CREATE UNIQUE INDEX IF NOT EXISTS uq_dz_seats_active_user
    ON arcade_doudizhu_room_seats (user_id) WHERE active;

-- 等待中的房间状态检索（匹配扫表）
CREATE INDEX IF NOT EXISTS idx_dz_rooms_waiting
    ON arcade_doudizhu_rooms (variant, stake, status, created_at);

-- ── 2) 局表扩 variant 列（AI 房也能开癞子桌） ─────────────────────────────
ALTER TABLE arcade_doudizhu_games
    ADD COLUMN IF NOT EXISTS variant text NOT NULL DEFAULT 'classic'
    CHECK (variant IN ('classic', 'wild'));

ALTER TABLE arcade_doudizhu_games
    ADD COLUMN IF NOT EXISTS room_id bigint
    REFERENCES arcade_doudizhu_rooms (id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_dz_games_room
    ON arcade_doudizhu_games (room_id);

-- ── 3) 设置键：PvP 房间超时/托管口径 ─────────────────────────────────────
INSERT INTO site_settings (name, value, descr, grp) VALUES
    ('doudizhu_pvp_idle_minutes', '2', 'PvP 房间无动作散桌（分钟）', 'module_fun')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max,
                           hint, group_key, card_order) VALUES
    ('doudizhu_pvp_idle_minutes', 'number', 'PvP 散桌时限',
     'PvP room idle timeout', '分', 1, 10,
     'waiting 房间超过该时长无人加入即散桌（退底分）；playing 房不适用',
     'module_fun', 21)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max,
      hint = EXCLUDED.hint,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

COMMIT;
