-- 0469 任务五档改名（前端 TIER_LABELS 同步换新档名）
-- 背景：NP 系「骨灰/走火入魔/烧糊涂/高烧/中烧」是上传量等级梗，站长
-- 要求换名。tier key 与英文 name 不动，只换中文展示；subtitle 是五档卡
-- 副标题行（前端 task-board-sections.tsx 直读 task.subtitle）。
-- 0027 已播库不可改，本迁移按 tier UPDATE。
UPDATE tasks SET subtitle = CASE tier
    WHEN 'master' THEN '传奇境界（原 VIP 专享档）'
    WHEN 'ultimate' THEN '大师之路（原 VIP 专享档）'
    WHEN 'extreme' THEN '资深磨练'
    WHEN 'veteran' THEN '进阶历练'
    WHEN 'insane' THEN '入门挑战'
    ELSE subtitle
END
WHERE tier IS NOT NULL;
