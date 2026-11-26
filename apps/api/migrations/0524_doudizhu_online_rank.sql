-- 0524 斗地主 PvP 在线状态 + 段位派生底座（2026-11-26）。
--
-- 在线状态：room_seats 增 last_seen（最近活跃时间）。join/state/ready
-- 动作时刷新；视图按「30s 内有更新」判定 online。纯展示用，不改撮合规则。
--
-- 段位：不建持久化段位表——等级从 arcade_doudizhu_games 的 settled 局
-- 实时派生（胜场/胜率/净分），刷新即一致，避免双写漂移；后续若要赛季
-- 星数沉淀再另建表。

BEGIN;

ALTER TABLE arcade_doudizhu_room_seats
    ADD COLUMN IF NOT EXISTS last_seen timestamptz NOT NULL DEFAULT now();

COMMENT ON COLUMN arcade_doudizhu_room_seats.last_seen IS
    '座位最近活跃时间；在线状态点按 30s 窗口判定';

CREATE INDEX IF NOT EXISTS idx_dz_seats_seen
    ON arcade_doudizhu_room_seats (room_id, last_seen);

COMMIT;
