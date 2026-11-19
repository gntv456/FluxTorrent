-- 0501 等级门槛四维扩展（站长反馈 2026-11-19）：下载量 / 做种体积 /
-- 做种数量 / 分享率。
--
-- 现行 class_rules 只有四维（上传量/下载次数/做种时长/账龄），站长
-- 要的「下载量」是 downloaded 字节、「做种体积」是在种总大小、「做种
-- 数量」是在种颗数、「分享率」是 uploaded/downloaded。
--
-- 口径设计：新列全部 NULL = 不参与判定（兼容存量：升级后所有站点
-- 行为零变化，站长按需开维度）。判定谓词统一
--   (r.min_x IS NULL OR s.x >= r.min_x)
-- 分享率内部存「万分比」整数（125 = 1.25）避免浮点与精度漂移；
-- downloaded=0 的新号分享率视为无穷大（放行）——与全站 ratio 口径
-- 一致（downloaded=0 不显 ratio 也不惩罚）。
--
-- 数据源（与 class_adj 引擎同源）：
--   downloaded      users.downloaded
--   seeding_volume  sum(torrents.size) WHERE snatches.seeding
--   seeding_count   count(*) WHERE snatches.seeding
--   ratio           uploaded / NULLIF(downloaded,0)

ALTER TABLE class_rules
    ADD COLUMN IF NOT EXISTS min_downloaded  BIGINT,
    ADD COLUMN IF NOT EXISTS min_seed_volume BIGINT,
    ADD COLUMN IF NOT EXISTS min_seed_count  INTEGER,
    ADD COLUMN IF NOT EXISTS min_ratio_bp    INTEGER;

COMMENT ON COLUMN class_rules.min_downloaded IS
    '最小下载量（字节；NULL=不参与判定）';
COMMENT ON COLUMN class_rules.min_seed_volume IS
    '最小在种体积（字节，snatches.seeding 现值聚合；NULL=不参与）';
COMMENT ON COLUMN class_rules.min_seed_count IS
    '最小在种颗数（snatches.seeding 现值计数；NULL=不参与）';
COMMENT ON COLUMN class_rules.min_ratio_bp IS
    '最小分享率（万分比整数，125=1.25；downloaded=0 视为∞放行；NULL=不参与）';
