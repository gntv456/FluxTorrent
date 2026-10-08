-- 0306（六轮审计 P0-1，2026-10-08，报告 _doc/转钟员六轮深测-反作弊落地与运行时-2026-10-08.md）
-- 近零重置频次限制：snatches.reset_count 记录该 (user, torrent) 已被认可的
-- 「近零重置」次数。真实客户端重启是低频事件（一年几次）；攻击链
-- 「报近零值重置锚点 → 等窗 → 报增量」每个循环可再铸 ≤ 速率钳 × 时窗的量。
-- 计费侧口径：24h 内首次近零重置认可（真实重启），之后的重置一律钉死旧锚
-- 点 + 记 cheat_events(reset:)。窗口判定在 worker 侧（reset_last_at 距今
-- < 24h 即视为频发），本迁移只补两列。
ALTER TABLE snatches
    ADD COLUMN IF NOT EXISTS reset_count integer NOT NULL DEFAULT 0;
ALTER TABLE snatches
    ADD COLUMN IF NOT EXISTS reset_last_at timestamptz;

COMMENT ON COLUMN snatches.reset_count IS
'已认可的累计读数近零重置次数（P0-1 频次限制的计数器：真实重启是低频事件，短窗内反复重置=搬基线攻击）';
COMMENT ON COLUMN snatches.reset_last_at IS
'上次认可近零重置的时点（与 reset_count 配合做 24h 频次判定；置位时若距上次 <24h 则重置不被认可）';
