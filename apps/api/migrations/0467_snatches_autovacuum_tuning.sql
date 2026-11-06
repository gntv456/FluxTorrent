-- snatches 的 autovacuum 调参（可见性图保鲜）。
--
-- 背景（2026-11-06 线上事故）：torrents 的 seeders/leechers/times_completed
-- 点刷走 snatches 的部分索引（0464/0466）做 index-only scan。但
-- snatches 是 announce 链路的高频 upsert 表（稳态 ~4,300 次/分），
-- 默认 autovacuum 阈值是 20% 行数（740k 行 ⇒ 14.8 万次变更才触发），
-- 可见性图长期不更新 ⇒ index-only scan 退化成回表：实测一次 5,000 个
-- 种子的点刷读了 3.1 万个磁盘块、耗时 104 秒，把 announce 的消费 tick
-- 整轮堵死。
--
-- 阈值调到 5% + 1 万行 ⇒ 约 4.7 万次变更触发一次（本机约 11 分钟），
-- 每轮 vacuum 约 16 秒，开销约 2%。代价换的是点刷从「百秒级回表」
-- 回到「秒级 index-only scan」。
--
-- 幂等：ALTER TABLE ... SET 可重复执行。
ALTER TABLE public.snatches SET (
    autovacuum_vacuum_scale_factor = 0.05,
    autovacuum_vacuum_threshold = 10000
);
