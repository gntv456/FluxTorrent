-- 0142 热点路径缺失索引补齐（全站代码审查 2026-09-20）
--
-- PostgreSQL 不会为外键自动建索引。以下列均为高频过滤/反查路径，此前只能顺序扫描：
--   * snatches.torrent_id：种子详情页做种/下载列表、补种候选、worker 回填
--     seeders/leechers/times_completed、做种体积统计。snatches 是全站最大表
--     （每 announce 更新），此前仅 (user_id, torrent_id) 主键——按 torrent_id
--     过滤走不到前缀，恒为全扫。
--   * comments：此前零二级索引。种子列表页每行一次 count(*) 子查询（torrents.rs
--     列表 SQL）、详情页评论、个人页评论，全部全表扫。
--   * messages.receiver_id：顶栏未读数每请求执行。
--   * topics(forum_id, last_post_at)：版面主题列表与计数。
--   * torrents.owner_id：发布者筛选、用户上传统计。
--
-- 注意：sqlx 迁移在事务内执行，无法用 CREATE INDEX CONCURRENTLY。对已积累千万行
-- 大表的存量站点，升级时本迁移会以 SHARE 锁阻塞写直至建完（announce 不受影响，
-- 走 tracker 内存表）；建议在低峰窗口部署，或带外用 CONCURRENTLY 预建同名索引
-- 后再启动（IF NOT EXISTS 会跳过）。

CREATE INDEX IF NOT EXISTS idx_snatches_torrent ON snatches (torrent_id);

CREATE INDEX IF NOT EXISTS idx_comments_torrent ON comments (torrent_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_comments_user ON comments (user_id, id DESC);

CREATE INDEX IF NOT EXISTS idx_messages_receiver_unread ON messages (receiver_id) WHERE unread;

CREATE INDEX IF NOT EXISTS idx_topics_forum_last ON topics (forum_id, last_post_at DESC);

CREATE INDEX IF NOT EXISTS idx_torrents_owner ON torrents (owner_id);
