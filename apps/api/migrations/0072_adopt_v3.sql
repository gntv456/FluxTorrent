-- 0072 借鉴清单落地（对照 _doc/全架构PT站功能域深度对比与借鉴-v3.md §27）：
--   ① ptppUserInfo 聚合端点依赖的做种体积统计（朱雀口径 seedingSize —— 本站首个持久列）
--   ② 晋升待遇：class_rules.promo_sparks（NP 升级送邀请口径 → 本站火花经济）
--   ③ H&R 预警：hr_snapshots.prewarned_at（U3D prewarn 口径：到期前 48h PM 提醒）
--   ④ H&R buffer 豁免：snatches.downloaded < size*10% 不建快照（U3D hitrun.buffer 口径）
--   ⑤ 闲置账号停用：users.dormant_at（U3D AutoDisableInactiveUsers 三段式，本站只做
--      「停用+登录拦截止告邮件」，不自动删除——教育站账号资产谨慎处理）

-- ① 做种体积物化：seeding_reward 每小时已扫全量做种行，顺手维护（省去 ptppUserInfo 的实时聚合）
ALTER TABLE users ADD COLUMN IF NOT EXISTS seeding_size BIGINT NOT NULL DEFAULT 0;

-- ② 晋升待遇（可配，0=不发）：worker class_auto_adjust 升级时发放 + 系统消息
ALTER TABLE class_rules ADD COLUMN IF NOT EXISTS promo_sparks BIGINT NOT NULL DEFAULT 0;
-- 默认阶梯：LV2 200 / LV3 500 / LV4 1000 / LV5 2000 / LV6 3000 / LV7 4000 / LV8 6000 /
--           LV9 8000 / LV10 10000 / LV11 15000 / LV12 20000（运营可在后台直接 UPDATE 调整）
UPDATE class_rules SET promo_sparks = CASE class_id
    WHEN 2 THEN 200 WHEN 3 THEN 500 WHEN 4 THEN 1000 WHEN 5 THEN 2000
    WHEN 6 THEN 3000 WHEN 7 THEN 4000 WHEN 8 THEN 6000 WHEN 9 THEN 8000
    WHEN 10 THEN 10000 WHEN 11 THEN 15000 WHEN 12 THEN 20000 ELSE 0 END
WHERE class_id BETWEEN 2 AND 12 AND promo_sparks = 0;

-- ③ H&R 预警标记（48h 内到期且未达标 → PM 一次；重置条件=satisfied/violated 自然终态）
ALTER TABLE hr_snapshots ADD COLUMN IF NOT EXISTS prewarned_at TIMESTAMPTZ;

-- ⑤ 闲置停用：90 天未登录且无做种且非员工/捐赠者 → dormant_at 打标 → 登录拦截。
--    不改 status（保留原始封禁语义），恢复由用户「任意登录尝试触发提醒邮件」+ 管理员一键恢复。
ALTER TABLE users ADD COLUMN IF NOT EXISTS dormant_at TIMESTAMPTZ;
