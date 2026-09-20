-- 迁移换号事故对齐脚本（2026-09-20）
--
-- 背景：开发期间 0127_play_generic_and_recycle / 0128_play_copy_tidy 因与并行会话的
-- forum_tip 撞号被让位到 0129/0130（并删除原 0127/0128 文件）。迁移文件一经应用即为
-- 不可变工件：任何应用过「旧号 0127_play/0128_play」的环境（CI、克隆、备份恢复）在
-- 拉到换号后的代码时会因 VersionMissing(128) 拒绝启动。现已回退换号：
--   * 0127_play_generic_and_recycle.sql / 0128_play_copy_tidy.sql 恢复原号原文件
--   * 0127_forum_tip.sql 挪到 0141（当时未提交过，无存量环境）
--
-- 本脚本只服务于「应用过换号序列」的环境（本机开发库，特征：_sqlx_migrations 里
-- 127 的 description = 'forum tip' 且不存在 128）。其余环境禁止执行。
--
-- 原理：sqlx 校验规则 = 「已应用版本必须存在同名文件且 checksum 一致」。删除三行
-- 记账后，下次 api 启动会按当前文件重放 127_play / 128_play / 141_forum_tip；
-- 三个文件全部幂等（CREATE TABLE/INDEX IF NOT EXISTS、守卫 UPDATE，可重复执行）。
--
-- 用法：psql -U flux -d fluxtorrent -f scripts/align_migration_renumber_0127_0128.sql

-- 仅当 127 记的是 forum tip（换号特征）才动手，对正常库是 no-op
DELETE FROM _sqlx_migrations
WHERE version = 127
  AND description = 'forum tip'
  AND NOT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = 128);

-- 换号期间的 play 两件套记账行（129/130）一并删除，改为按 0127/0128 原号重放
DELETE FROM _sqlx_migrations
WHERE version IN (129, 130)
  AND description IN ('play generic and recycle', 'play copy tidy')
  AND NOT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = 128);
