-- 0509 迁移纪律善后：契约运营面板行 min_class 99→98
--
-- 背景（2026-11-20 百灵站宕机复盘）：
--   f06c024a 曾把「已应用」的 0499 迁移文件改内容（min_class 99→98+注释），
--   sqlx 逐行校验 checksum 不符 → dev.38 webhook 自动发版时百灵库
--   flux-api 崩溃循环，全站 API 500 六小时。
--   修复口径（迁移只增不改）：
--     1) 仓库里 0499 恢复为原始已应用内容（ checksum 冻结）；
--     2) min_class 修正挪到本迁移，幂等可重放；
--     3) 两站库 499 checksum 按 0499 原始文件值对齐（运维 UPDATE，不入迁移）。
--
-- 本迁移只做 0499 首版想做的数据修正：契约运营面板行对齐同组
-- economy/onboarding=98——首版误写 99，把维护开发者（98）挡在门外
-- （端点权限 settings.view 矩阵授 class≥90，98 是合法维护档）。
-- 好学库已热修同值，此处幂等防漂移；百灵库 499 首版生效时写的是 99，
-- 由本迁移真正落到 98。

UPDATE staff_panel_entries
   SET min_class = 98
 WHERE tab_key = 'contractstats'
   AND min_class = 99;
