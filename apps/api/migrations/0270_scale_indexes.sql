-- 0270 规模化缺失索引补齐 + 后台巡检参数登记（2026-10-02 ZT81 压测）
--
-- 背景：以 2,000 用户 / 20,000 种子 / 42 万 snatches 的合成规模实测后发现，
-- 以下列均为高频过滤/排序路径却**完全无索引**，数据量上来后退化为顺序扫描
-- （详见 _doc/资深PT站长深度体验测试与优化建议-2026-10-02.md）。
--
-- 注意：sqlx 迁移在事务内执行，无法用 CREATE INDEX CONCURRENTLY。大表存量站点
-- 升级时本迁移会以 SHARE 锁阻塞写直至建完；建议低峰窗口部署，或带外用
-- CONCURRENTLY 预建同名索引后再启动（IF NOT EXISTS 会跳过）。
--
-- 索引清单一览（每条先写「它救的是哪个查询」）：
--   ① messages 收件箱：community_http/message.rs 默认列表（WHERE receiver_id,
--      location, ORDER BY id DESC）——现有 idx_messages_receiver_unread 是
--      `WHERE unread` 的**部分**索引，不带 unread 谓词的查询命中不了 → 全表扫。
--   ② messages 发件箱：WHERE sender_id AND saved，此前 sender_id 零索引。
--   ③ messages 分文件夹：WHERE receiver_id, folder。
--   ④⑤ users 模糊搜索：admin_http/user_list.rs 的 username/email ILIKE、
--      http/plugins.rs 的勋章墙 username ILIKE。pg_trgm 此前只建在 torrents 上。
--      （username/email 是 citext；实测 gin_trgm_ops 可直接作用于 citext。）
--   ⑥ users 活跃统计：admin_http/overview.rs 的 WAU（last_seen_at 无索引）。
--   ⑦⑧ audit_log：admin_http/audit.rs 的 action ILIKE 与链校验（此前只有主键）。
--   ⑨ snatches leeching 谓词：首页三态统计 WHERE leeching（现有索引只覆盖 seeding）。
--   ⑩⑪ 后台每分钟巡检的谓词：sweep_stale_peers（按 last_seen_at 清理在线标记）、
--      backfill_pieces_hash（WHERE pieces_hash IS NULL）——两条原本都是全表扫。
--   ⑫ posts 正文搜索：community_http/forums.rs 的全局搜索 body_text ILIKE；
--      posts 是分区表，PG12+ 会在各分区自动建同名分区索引。

BEGIN;

-- ① messages 收件箱（location=1 为收件箱域；按 id DESC 翻页）
CREATE INDEX IF NOT EXISTS idx_messages_receiver_live
    ON messages (receiver_id, id DESC) WHERE location = 1;

-- ② messages 发件箱（saved=1 为已存副本）
CREATE INDEX IF NOT EXISTS idx_messages_sender_saved
    ON messages (sender_id, id DESC) WHERE saved = 1;

-- ③ messages 按文件夹
CREATE INDEX IF NOT EXISTS idx_messages_folder
    ON messages (receiver_id, folder, id DESC) WHERE location = 1;

-- ④⑤ users 模糊搜索（trgm 支持 ILIKE '%x%'）
CREATE INDEX IF NOT EXISTS idx_users_username_trgm
    ON users USING gin (username gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_users_email_trgm
    ON users USING gin (email gin_trgm_ops);

-- ⑥ users 活跃统计
CREATE INDEX IF NOT EXISTS idx_users_last_seen
    ON users (last_seen_at) WHERE last_seen_at IS NOT NULL;

-- ⑦⑧ audit_log 查询与模糊过滤
CREATE INDEX IF NOT EXISTS idx_audit_actor_created
    ON audit_log (actor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_action_trgm
    ON audit_log USING gin (action gin_trgm_ops);

-- ⑨ snatches：leeching 谓词（首页在线统计）
CREATE INDEX IF NOT EXISTS idx_snatches_leeching
    ON snatches (user_id) WHERE leeching;

-- ⑩ snatches：在线清理谓词（sweep_stale_peers 每分钟一轮）
CREATE INDEX IF NOT EXISTS idx_snatches_last_seen_active
    ON snatches (last_seen_at) WHERE seeding OR leeching;

-- ⑪ torrents：pieces_hash 回填谓词（backfill_pieces_hash 每分钟一轮）
CREATE INDEX IF NOT EXISTS idx_torrents_missing_pieces
    ON torrents (id) WHERE pieces_hash IS NULL;

-- ⑫ posts 正文搜索（分区表 → 各分区自动建同名索引）
CREATE INDEX IF NOT EXISTS idx_posts_body_trgm
    ON posts USING gin (body_text gin_trgm_ops);

-- 后台 H&R 巡检的回看窗进设置面板（代码缺省 6h，见 worker/src/jobs/hr.rs）。
-- 语义：hr_enforce 只扫「近 N 小时完成的 snatch」，替代原先的全表扫；
-- 值调到很小可能漏掉事件积压补跑，故 min 给 1h、max 给 30 天。
-- 归到既有「系统调度」卡（与 announce_interval 同卡），站长一眼能找到。
INSERT INTO site_settings (name, value, descr, grp) VALUES
    ('hr_scan_lookback_hours', '6',
     'H&R 巡检回看窗口（小时）：只扫描近 N 小时内完成的下载', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, unit, min, max, group_key, card_order)
VALUES
    ('hr_scan_lookback_hours', 'number', 'H&R 巡检回看窗口',
     'H&R scan lookback', '小时', 1, 720, '系统调度', 60)
ON CONFLICT (name) DO UPDATE
    SET type = EXCLUDED.type,
        label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
        unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max,
        group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- 自检：12 条索引必须全部落地（少一条说明被静默跳过，早失败早发现）
DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM pg_indexes
     WHERE schemaname = 'public'
       AND indexname IN (
           'idx_messages_receiver_live', 'idx_messages_sender_saved',
           'idx_messages_folder', 'idx_users_username_trgm',
           'idx_users_email_trgm', 'idx_users_last_seen',
           'idx_audit_actor_created', 'idx_audit_action_trgm',
           'idx_snatches_leeching', 'idx_snatches_last_seen_active',
           'idx_torrents_missing_pieces', 'idx_posts_body_trgm');
    IF n < 12 THEN
        RAISE EXCEPTION '规模索引只建成 % / 12 条', n;
    END IF;
    RAISE NOTICE '规模索引已补齐：% 条', n;
END $$;

COMMIT;
