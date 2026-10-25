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

-- 2026-10-20 复发：同型事故第二次（288/376/388/389/390/391）。
-- 起因：这几条迁移曾在**本地 CRLF 工作区**被 apply（编辑器另存为 CRLF，
--      .gitattributes 的 `*.sql text eol=lf` 只管 checkout/commit，管不住
--      编辑器另存），库里记下的是 CRLF 字节的 sha384；而仓库 canonical 形态
--      是 LF（index/blob=i/lf）。于是：
--        · 本机工作区构建 → 校验和恰好对上 → 能起；
--        · 干净检出/CI/官方镜像（LF）→ 对不上 → api 启动即
--          `migration NNN was previously applied but has been modified`，
--          worker 依赖 api 也一起起不来（2026-09-16 那次是 28/54/57/58/59/61，
--          本次是 288/376/388/389/390/391 —— 同一类事故复发，说明只修数据
--          不够，得把「迁移文件在库里必须按 canonical LF 记账」写成硬规矩）。
-- 口径：这里记的 hash = **仓库 canonical（LF）字节**的 sha384，
--      与干净检出/CI 构建算出来的一致 ⇒ 任何检出形态都能启。
-- 幂等：值恒定，重复执行无害。
UPDATE _sqlx_migrations SET checksum = '\xcf609bd49986d66eff940570dfd7ce2008b05d810fbdb30276fee557a27553d984c137442fa9c220ec28b021478117c6' WHERE version = 288;
UPDATE _sqlx_migrations SET checksum = '\x11bfb2605b49c45ccf288056ba2d291f937dc1529b617d0450b4f107ffdd94c275885254a38c13c3b24d9fdf05113660' WHERE version = 376;
UPDATE _sqlx_migrations SET checksum = '\x9e0e89edb3f5713adf803df2ae0ffb5d7d057bb830a7b76cab493a4419061488c7c919448cf2eaed2a0fd4ba10926a47' WHERE version = 388;
UPDATE _sqlx_migrations SET checksum = '\x06ecd1d754616ca4bcb2d9650f20e874d3ebe125e91f04489a4d8961efe1442280b75707b749ef1b8b3af7f5af185e99' WHERE version = 389;
UPDATE _sqlx_migrations SET checksum = '\xf859296bf292877b62b7cac1185c11d0c8afb2436b5a2a013cd6a2ad91b6ca12020b346bcdebe950e1200a769e8b1fb1' WHERE version = 390;
UPDATE _sqlx_migrations SET checksum = '\x76723dc5142e294b1c49a644937beaf189923a18438b053c0afeab4e52316e4937976d8f3d8b9f4f5d1a89a6d486498f' WHERE version = 391;
COMMIT;
