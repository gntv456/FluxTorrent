-- 0134 做种结算的两处收尾：① 结算 JOIN 的部分索引；② 函数注释澄清（ln 与 log10 等价）
--
-- 背景（0133 落地后的自查）：
--   1) 每小时结算扫 `snatches WHERE seeding`，而 snatches 只有主键 (user_id, torrent_id)，
--      做种行占比很小却要全表扫。加**部分索引**只覆盖做种行 —— 索引体积小、announce 写入代价低。
--   2) 0133 的函数体用的是 `ln(...)/ln(1+vol_base)`，注释里写的是 log10 —— 两者比值完全等价
--      （换底公式），但字面不一致会让后人怀疑口径漂移。这里用 COMMENT 说明清楚。
--   3) 另记录调用注意：`seeding_params()` 是 STABLE 函数，调用侧务必
--      `WITH p AS MATERIALIZED (SELECT * FROM seeding_params())`，否则优化器可能把它内联进
--      nestloop 内层 → 每颗种子读 8 次 site_settings。

CREATE INDEX IF NOT EXISTS idx_snatches_seeding ON snatches (user_id) WHERE seeding;

-- 本文件原先对 seeding_torrent_bonus / seeding_hourly / seeding_params 下 COMMENT，
-- 但这三个函数都在 0136 才 CREATE —— 空库跑到这里必报
-- 「function seeding_torrent_bonus(...) does not exist」，装站直接卡死（四审复验空库首启时踩出，
-- 存量库因当年手工建过函数才掩盖住）。三段注释已整体移到 0136 的函数定义之后，
-- 语义不变（COMMENT 幂等），存量库执行 0136 时重复打注释无害。
