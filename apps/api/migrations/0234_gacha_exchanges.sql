-- 0234_gacha_exchanges.sql — G31-C：定向兑换（方案《抽卡玩法落地方案-2026-09-27》§3 0234）
--
-- 限次直接落在唯一键上（UNIQUE(user_id, card_id, season_key)），不靠代码记——
-- 并发双兑由数据库兜底；season_key 缺省 'all'（无赛季概念时的全期一次）。

CREATE TABLE gacha_exchanges (
    id           bigserial PRIMARY KEY,
    user_id      bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    card_id      bigint NOT NULL REFERENCES gacha_cards(id) ON DELETE CASCADE,
    season_key   text NOT NULL DEFAULT 'all',
    shards_spent int NOT NULL CHECK (shards_spent > 0),
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, card_id, season_key)
);
