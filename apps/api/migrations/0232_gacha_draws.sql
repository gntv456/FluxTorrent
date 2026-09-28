-- 0232_gacha_draws.sql — G31-B：持有/图鉴与抽卡流水（方案 §3 0232–0233）
--
-- 图鉴纪律：lit_at 一旦写入不因任何后续操作清除（「图鉴只增不减」落在列上——
-- 分解只动 held，不动 lit_at；G31-C 的分解路径必须遵守）。
-- 流水纪律：一次请求 N 行共用 idempotency_key，UNIQUE(idempotency_key, seq)
-- 是「同 key 只出一批」的数据库级兜底；seed 落库以便事后复核抽取序列。

CREATE TABLE gacha_user_cards (
    user_id      bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    card_id      bigint NOT NULL REFERENCES gacha_cards(id) ON DELETE CASCADE,
    held         int NOT NULL DEFAULT 0 CHECK (held >= 0),
    lv           smallint NOT NULL DEFAULT 1 CHECK (lv >= 1),
    lit_at       timestamptz NOT NULL,
    first_source text NOT NULL,
    PRIMARY KEY (user_id, card_id)
);

CREATE TABLE gacha_draws (
    id          bigserial PRIMARY KEY,
    user_id     bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    banner_id   bigint NOT NULL REFERENCES gacha_banners(id) ON DELETE CASCADE,
    seq         int NOT NULL,
    drawn_at    timestamptz NOT NULL DEFAULT now(),
    output_type text NOT NULL,
    rarity      text,
    card_id     bigint,
    shards      int NOT NULL DEFAULT 0,
    pity_at     int NOT NULL,                -- 该抽时「未出金连续抽数」（含本抽）
    was_pity    bool NOT NULL DEFAULT false, -- 本抽是否硬保底强制
    is_new      bool NOT NULL DEFAULT false, -- 卡牌档：是否首次点亮图鉴
    seed        bigint NOT NULL,             -- 本抽随机种子（事后复核）
    idempotency_key text NOT NULL,
    UNIQUE (idempotency_key, seq)
);
CREATE INDEX gacha_draws_user_banner_idx ON gacha_draws (user_id, banner_id, seq DESC);
