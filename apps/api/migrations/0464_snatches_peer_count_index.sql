-- 0464: announce 消费慢语句加速（做种数滞后 / 事件流积压）
--
-- 现象（2026-11-06 线上实测）：
--   · redis flux:announce 流长度 12,202 → 12,332（60 秒净增 130），
--     消费组 fluxcg-announce 的 lag 长期挂 1.1 万条；
--   · worker 每批消费后要重算受影响种子的 peer 统计：
--       UPDATE torrents t SET seeders = (SELECT count(*) FROM snatches s
--           WHERE s.torrent_id = t.id AND s.seeding), …
--     实测 196 个种子一次 UPDATE 耗 5.6~8.1 秒（slow_statement 告警），
--     消费吞吐被压到 batch(200) × 每分钟一次 = 200 条/分 < 生产 330 条/分
--     ⇒ 事件无限积压，用户资料页「做种数」长期显示十几小时前的旧值
--     （有人显示 0：其 announce 事件全堵在流里，snatches.seeding 未被置真）。
--
-- 修法：给 snatches 补两条**部分索引**——只索引 seeding / leeching 的行，
-- count(*) 可直接走索引（原来是 (torrent_id) 全量索引 + 回表过滤）。
-- 迁移文件里不能用 CONCURRENTLY（sqlx migrate 每个文件跑在一个事务里，
-- CONCURRENTLY 不允许在事务内执行）；74 万行建索引秒级，可接受。
--
-- 回滚：DROP INDEX IF EXISTS idx_snatches_torrent_seeding;
--        DROP INDEX IF EXISTS idx_snatches_torrent_leeching;

CREATE INDEX IF NOT EXISTS idx_snatches_torrent_seeding
    ON public.snatches (torrent_id) WHERE seeding;
CREATE INDEX IF NOT EXISTS idx_snatches_torrent_leeching
    ON public.snatches (torrent_id) WHERE leeching;
