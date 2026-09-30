-- 宠物养成（2026-09-30）· 娱乐屋第 9 款
--
-- 经济定位：**回收口**。宠物「卖萌」产出的魔力只来自你喂进去的「能量」，
-- 且返还比例恒 < 1000‰（等级越高封顶 950‰）—— 最多把你喂的 95% 还给你，
-- 永远不会增发。它的意义是**养成**（等级/外观），不是套利。
--
-- 时间推进不落定时任务：每次读取/喂食/领取时，按 `last_tick` 到「此刻」的间隔
-- 结算一次（消化能量进 pending、衰减饥饿）。这样不依赖 worker，也不会漏算。

BEGIN;

CREATE TABLE IF NOT EXISTS arcade_pets (
    user_id      bigint PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    species      text NOT NULL DEFAULT 'slime',
    name         text NOT NULL DEFAULT '',
    level        integer NOT NULL DEFAULT 1,
    exp          bigint NOT NULL DEFAULT 0,
    hunger       integer NOT NULL DEFAULT 100,   -- 0..100，纯展示/养成
    energy       bigint NOT NULL DEFAULT 0,      -- 未消化的能量（喂食累积，封顶）
    pending      bigint NOT NULL DEFAULT 0,      -- 已消化、待领取的魔力
    last_tick    timestamptz NOT NULL DEFAULT now(),
    last_feed_at timestamptz,
    created_at   timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT arcade_pets_level_range  CHECK (level BETWEEN 1 AND 10),
    CONSTRAINT arcade_pets_hunger_range CHECK (hunger BETWEEN 0 AND 100),
    CONSTRAINT arcade_pets_amounts     CHECK (energy >= 0 AND pending >= 0)
);

DO $$
BEGIN
    RAISE NOTICE '宠物养成就绪：产出返回率恒 < 1000‰（回收口）';
END $$;

COMMIT;
