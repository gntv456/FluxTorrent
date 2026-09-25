-- FluxTorrent 存量库迁移 checksum 对齐（一次性；0134/0136「函数注释搬移」修复的配套）
--
-- 背景：`0134_seeding_settle_tuning.sql` 原本对 `seeding_torrent_bonus` / `seeding_hourly`
--   / `seeding_params` 下 COMMENT，但这三个函数都在 **0136** 才 CREATE。空库按序执行到
--   134 必报 `function seeding_torrent_bonus(...) does not exist`，flux-api 直接 crash-loop
--   ——即「装站装不起来」。存量库因函数早已存在而把这个断链掩盖了三年/几天（0134 最后
--   改动是 2026-09-20 a01ae44），而 master 的 CI 自 2026-09-22 起空转，所以没被拦下。
--   本次复验用 `docker compose down -v` 清卷才撞出来。
--
-- 修复：三段 COMMENT 整体移到 0136 的函数定义之后（`COMMENT` 幂等，存量库重复执行无害）。
--   代价是 **两个迁移文件的内容变了**：已应用过 134/136 的库下次启动会报
--   "migration 134 was previously applied but has been modified" 而拒绝启动。
--
-- 用法：升级到含本修复的镜像之前，对站点库执行一次（值取自一次干净装站的实测记账）：
--   docker exec -i flux-postgres psql -U flux -d fluxtorrent \
--     < scripts/align_migration_checksums_0134.sql
-- 幂等：重复执行无害。只改 _sqlx_migrations 的记账值，不动业务数据、函数定义或注释。
-- 自检：执行后 `SELECT version, checksum FROM _sqlx_migrations WHERE version IN (134,136);`
--   应与下面两行一致。
BEGIN;
UPDATE _sqlx_migrations SET checksum = '\xeb0dce25f14ba3c6c9635c57329f7a341ff08183c9173c1c1f5b625c9c05ba8f2085516925dd1e0fe0b3cd9e85740d7d' WHERE version = 134;
UPDATE _sqlx_migrations SET checksum = '\x9e69d6b6a1fdf23346a00985ebed2e5efbb4e5c39616de48219c81086c941795c8f72a0b333cf54487395e460d2eccae' WHERE version = 136;
COMMIT;
