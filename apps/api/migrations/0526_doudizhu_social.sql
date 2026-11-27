-- 0526 斗地主房间社交底座：可见性 / 观战 / 房间频道消息（2026-11-26）。
--
-- 可见性：public 进公开匹配列表；private 不进列表，凭房号进入。本轮只做
-- 标签与列表过滤；加入口令（join_code）字段预留，验口令入座的口径待定稿后
-- 再开，HTTP 层本轮不校验 join_code。
--
-- 观战：非座位身份旁观当前房间/牌局。不占 3 座、不扣底分。一人一房一条
-- 记录，退出即删；观战计数 = 有效观战行数。散桌/退房级联清理。
--
-- 房间消息：waiting/playing 房内座位 + 旁观可见。按时间倒序拉最近若干条；
-- 禁言用户（users.status=1）由 HTTP 层拦截发送，表内不重复标记。
-- 消息体长度与敏感词在 repo/HTTP 层收口，表只存正文。

BEGIN;

-- ── 1) 房间可见性（标签 + 预留口令） ──────────────────────────────────────
ALTER TABLE arcade_doudizhu_rooms
    ADD COLUMN IF NOT EXISTS visibility text NOT NULL DEFAULT 'public'
    CHECK (visibility IN ('public', 'private'));

ALTER TABLE arcade_doudizhu_rooms
    ADD COLUMN IF NOT EXISTS join_code text;

COMMENT ON COLUMN arcade_doudizhu_rooms.visibility IS
    'public=进公开匹配列表；private=仅凭房号进入。口令 join_code 预留未启用';

COMMENT ON COLUMN arcade_doudizhu_rooms.join_code IS
    '预留：私密房加入口令；本轮不参与入座校验，待产品口径定稿后启用';

-- ── 2) 观战席 ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS arcade_doudizhu_spectators (
    room_id   bigint NOT NULL
              REFERENCES arcade_doudizhu_rooms (id) ON DELETE CASCADE,
    user_id   bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    joined_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (room_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_dz_spec_room
    ON arcade_doudizhu_spectators (room_id);

CREATE INDEX IF NOT EXISTS idx_dz_spec_user
    ON arcade_doudizhu_spectators (user_id);

COMMENT ON TABLE arcade_doudizhu_spectators IS
    '斗地主观战：非座位旁观；不扣底分不占座；退房/散桌级联清理';

-- ── 3) 房间频道消息 ───────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS arcade_doudizhu_room_messages (
    id         bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    room_id    bigint NOT NULL
               REFERENCES arcade_doudizhu_rooms (id) ON DELETE CASCADE,
    sender_id  bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    body       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_dz_msg_room
    ON arcade_doudizhu_room_messages (room_id, created_at DESC);

COMMENT ON TABLE arcade_doudizhu_room_messages IS
    '斗地主房间频道消息；禁言(status=1)用户由 HTTP 层拦截发送';

COMMIT;
