-- 优先级① 核心行为联动轻量版：口粮券 + 渔汛做种门槛
-- 设计要点（见 _doc/PT游戏-玩法完善与数值平衡-2026-09-29.md §1）：
--   1. 口粮券是「行为联动资源」：每日做种满 6h 发放，抵养成喂养 / 鱼竿升级消耗，
--      纯消耗侧、不返魔力，结构上杜绝通胀源。
--   2. 「做种满 6h」判定**复用 seeding_reward 流水**（幂等键 = seeding:{user}:{yyyymmddhh}），
--      当日不同小时段条数 = 当日做种小时数。不新增任何做种时长累计表、零风险不改资金管线。
--   3. 发放幂等靠 food_coupon_grants(user_id, day) 主键；job 重跑 / 跨日补跑安全。

-- 口粮券余额挂在 users 上（轻量，单字段）
ALTER TABLE users ADD COLUMN food_coupons integer NOT NULL DEFAULT 0;

-- 口粮券每日发放幂等记录：每用户每自然日（站点 UTC+8）至多 1 张
CREATE TABLE food_coupon_grants (
    user_id bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    day date NOT NULL,
    granted_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, day)
);

-- 口粮券核销幂等（防同键重放白嫖；与用户行锁配合防并发双花）
CREATE TABLE coupon_uses (
    idem text NOT NULL PRIMARY KEY,
    user_id bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    used_at timestamptz NOT NULL DEFAULT now()
);

-- 行为联动查询加速：按 kind + 时间范围扫 seeding_reward 流水（日/周做种小时统计）
CREATE INDEX idx_spark_seeding_day
    ON spark_ledger (created_at, user_id)
    WHERE kind = 'seeding_reward';

COMMENT ON COLUMN users.food_coupons IS
    '口粮券余额：做种满 6h/日发放，抵养成喂养/鱼竿升级消耗（纯行为联动资源，不返魔力）';
COMMENT ON TABLE food_coupon_grants IS
    '口粮券每日发放幂等记录（每用户每自然日至多 1 张）';
COMMENT ON TABLE coupon_uses IS
    '口粮券核销幂等表（防同键重放）';
