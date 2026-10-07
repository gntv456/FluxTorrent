-- 0296 装扮过期前置（三轮审计 P2，2026-10-07）
--
-- 背景：user_dressups 只有 created_at，无过期概念——当前全部装扮 SKU 为
-- 永久制所以无实际影响，但「限时头像框」（gacha/活动发放的常见形态）一旦
-- 上线，佩戴将永不过期（勋章体系当初的同款缺陷前置补上）。
--
-- 方案：expires_at 可空列（NULL=永久，既有行全部 NULL 语义不变）；
-- 佩戴读侧（dressup.rs 的佩戴列表/生效渲染）过滤过期行；到期自动摘除
-- 交给读侧过滤即可（低频装饰数据，无需清扫 job——与勋章 sweep 的差异：
-- 勋章有角标计数需求才上 job，这里纯展示）。
ALTER TABLE user_dressups
    ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;

-- 发放侧（shop_effects/gacha 等未来限时 SKU）写入 expires_at 即生效。
-- 存量行不回填（NULL=永久）。
