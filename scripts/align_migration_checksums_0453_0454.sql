-- FluxTorrent 存量库迁移 checksum 对齐（一次性，2026-11-02 行尾事故修复 · 第三次复发）
-- 背景：同一类事故第三次复发（2026-09-16 是 28/54/57/58/59/61，2026-10-20 是
--      288/376/388/389/390/391，本次是 453/454）。
--      起因与前两次完全同型：这两条迁移文件在**本地 CRLF 工作区**被 apply
--      （编辑器另存为 CRLF；.gitattributes 的 `*.sql text eol=lf` 只管
--      checkout/commit，管不住编辑器另存），库里记下的是 CRLF 字节的
--      sha384；而仓库 canonical 形态是 LF（index/blob=i/lf）。于是：
--        · 本机工作区构建 → 校验和恰好对上 → 能起；
--        · 干净检出 / CI / 官方镜像（LF）→ 对不上 → api 启动即
--          `migration NNN was previously applied but has been modified`，
--          api 崩溃循环、worker 依赖 api 也一起起不来。
-- 触发经过：2026-11-02 发 v0.5.6（手机端首页重设计 + 签到万年历），CI 出的
--      镜像带 LF 的 0453/0454，api 起不来；update.sh 因 docker/.flux-version
--      为空（无线上版本记录）无可回滚目标，直接中断。
-- 口径：这里记的 hash = **仓库 canonical（LF）字节**的 sha384，
--      与干净检出/CI 构建算出来的一致 ⇒ 任何检出形态都能启。
--      两条文件除行尾外内容逐字节相同（已核：0453 CRLF 5422 → LF 5321、
--      0454 CRLF 1615 → LF 1589，LF 化后与 blob 完全一致），schema 无差异。
-- 幂等：值恒定，重复执行无害。
-- 用法：升级到 v0.5.6+ 之前，对 FluxTorrent 库执行一次：
--   docker exec -i flux-postgres psql -U flux -d fluxtorrent < this.sql
BEGIN;
UPDATE _sqlx_migrations SET checksum = '\xa12aa7abf249ce66039b4e3b60afdee5d77b07a1e89d5927e0166c461ffb9c8811d1914f801ba39e785946cc627020f1' WHERE version = 453;
UPDATE _sqlx_migrations SET checksum = '\x1774b162e3b2b782d71e609014e81e71122a5a9916dc40e3539a1763794bc9242588f00899c74e816aa96d31fc662a90' WHERE version = 454;
COMMIT;
