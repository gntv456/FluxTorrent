-- FluxTorrent 存量库迁移 checksum 对齐（一次性，2026-09-16 行尾事故修复）
-- 背景：6 个迁移曾以 CRLF 行尾被 apply，官方镜像（LF）校验不过报
--      "migration XX was previously applied but has been modified"。
-- 用法：升级到官方镜像 v0.1.2+ 之前，对 FluxTorrent 库执行一次：
--   docker exec -i flux-postgres psql -U flux -d fluxtorrent < this.sql
-- 幂等：重复执行无害（值恒定）。
BEGIN;
UPDATE _sqlx_migrations SET checksum = '\x57e7190e511c1f19ba8bdc90300fb5d7306f95c7db962c32bfac170507a7557315bbe300308bab55aa0387183ca982d7' WHERE version = 28;
UPDATE _sqlx_migrations SET checksum = '\x8cf3c92c45c7b9d8d6c88cece11df46c71f39497334b2096be51be4d76f060393693c7b901f61fe6fdc40fcd0471c8bf' WHERE version = 54;
UPDATE _sqlx_migrations SET checksum = '\x6d9acffb8a395d61152a1dda8aee1ffeb3a2c02cfaf876f02878b2e527beef4b756e56590f125f20f2b1a9d328bfb93a' WHERE version = 57;
UPDATE _sqlx_migrations SET checksum = '\x1536196b1801efbb3e24472b7c3ccb4184d38a0dff5bfe3905ca2487a3b4f86c2f7def43887ee149e448269b45da6537' WHERE version = 58;
UPDATE _sqlx_migrations SET checksum = '\xc3ff37e4388283106ca31e707e4b530daac19d4c3c06ebd5a2032ef200466fbac2252e42bf1ab461f4384517a13a476a' WHERE version = 59;
UPDATE _sqlx_migrations SET checksum = '\x6a751326d3e26679daff7c323f9f02a96045951bda0167b6a8aa1c867baafc887f716e9dcd8748f7bf3a4206356dec41' WHERE version = 61;
COMMIT;
