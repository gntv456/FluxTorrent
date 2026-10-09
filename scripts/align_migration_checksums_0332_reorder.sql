-- FluxTorrent 存量库迁移 checksum 对齐（一次性，2026-10-09 0332 断链修复）
--
-- 背景：第三次撞号让号（6b4a635）把建 content_networks 的迁移从 0330
--      挪到 0333，但 0332_network_subscriptions 的外键引用留在原位 ⇒
--      全新空库跑到 0332 即崩（relation "content_networks" does not
--      exist，装机第一步就死）。修复：0332 头部前置同构的
--      CREATE TABLE IF NOT EXISTS 建表段（数据回填仍归 0333）⇒
--      332 文件内容变 ⇒ 已应用 0332 的存量库 checksum 失配，api
--      启动报 "migration 332 was previously applied but has been
--      modified"。本脚本把库内 332 记录对齐到修复后的文件。
--
-- 用法：升级到含本修复的镜像之前，对库执行一次：
--   docker exec -i flux-postgres psql -U flux -d fluxtorrent < this.sql
-- 幂等：重复执行无害（值恒定）。
-- 口径：checksum 以 **LF 字节** 的 sha384 计（镜像从干净检出构建，
-- 与 audit_migration_checksums.py 的判据一致；本文件当前全 LF，
-- 无 CRLF 歧义）。
BEGIN;
UPDATE _sqlx_migrations
SET checksum = '\x682fca9350a210b77a80247382df3290b98618685464695bbd1997979012559830e77a4cbdae475be1ea39a1ed1d97cf',
    description = 'network subscriptions'
WHERE version = 332;
COMMIT;
