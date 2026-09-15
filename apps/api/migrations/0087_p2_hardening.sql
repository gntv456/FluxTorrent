-- 0087: P2 健壮性批次配套迁移（原号 0085 与 section_kinds 撞号改 87；本库已按 87 应用）。
--
-- 1) H&R 豁免阈值统一：hr_enforce 豁免线 = size/10（10%），券核销线 = size*104/1000
--    （≈10.4%，注释误写 4%）。10%~10.4% 之间的用户会被判违规同时券未核销。
--    无表结构变更：两侧 SQL 已在代码中统一为同一表达式（t.size * 104 / 1000），
--    此处仅作迁移占位说明，保证部署清单里能看到该口径变更。

-- 2) Redis 撤销线持久化兜底：logout_nbf 只存 Redis（TTL 24h），Redis 重启即令
--    已撤销 token 复活。改为 DB 权威表（require_auth 读库，Redis 只做缓存加速）。
CREATE TABLE IF NOT EXISTS token_revocations (
    user_id BIGINT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    nbf     BIGINT NOT NULL,           -- iat <= nbf 的 JWT 一律失效
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 存量：把 Redis 中仍存活的撤销线搬进库（由应用启动时执行，SQL 侧无法访问 Redis；
-- 未搬运的旧线最多 24h 后自然过期——与 JWT 本身 24h 有效期一致，无安全回归）。

-- 3) 商店购买效果补发标记：扣款成功→效果执行失败→重试时 spend 返回 Replayed、
--    效果被跳过（花钱买空气）。加 effect_applied 列，shop_buy 用 CAS 置位决定是否执行效果。
ALTER TABLE shop_orders ADD COLUMN IF NOT EXISTS effect_applied BOOLEAN NOT NULL DEFAULT FALSE;
