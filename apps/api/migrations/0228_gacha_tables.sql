-- 0228_gacha_tables.sql — G31-A 骨架：抽卡四表（方案《抽卡玩法落地方案-2026-09-27》§3）
--
-- 设计约束：机制进代码、内容进包、视觉与经济参数走行表；产出类型用判别列
-- （output_type）而不是并列固定列。稀有度视觉 token 照 medal_rarities 0143 先例
-- 行表下发——前端不得再持任何色表（样张阶段已实测：同一份 frame SVG 只换
-- --r-* token 即可出 5 种卡面）。
--
-- 种子 = 样张 demo 池（gacha_math_vectors.json 的 demo_pool 用例）逐位同源：
-- 公示端点（gacha_http::rates）按本种子重建 RateRow 后喂 crate::gacha_math，
-- 输出必须与向量逐位一致（方案 §5 断言「公示页七行概率与 economics() 逐位一致」
-- 的验证基础）。种子是「站长可改的缺省内容」，不是机制。

-- 1) 稀有度 token 行表（视觉参数全在行上；gold_rank 是数不是枚举——「出金」阈值）
CREATE TABLE gacha_rarities (
    key        text PRIMARY KEY,
    label      text NOT NULL,
    sort       int  NOT NULL,
    stars      smallint NOT NULL,
    gold_rank  smallint NOT NULL DEFAULT 3,
    frame      text,
    frame_hi   text,
    glow       text,
    bg1        text,
    bg2        text,
    ink        text,
    foil       numeric(4,3) NOT NULL DEFAULT 0,
    foil_mask  text NOT NULL DEFAULT 'art'
               CHECK (foil_mask IN ('art','frame','none')),
    enabled    bool NOT NULL DEFAULT true
);

INSERT INTO gacha_rarities (key, label, sort, stars, gold_rank,
    frame, frame_hi, glow, bg1, bg2, ink, foil, foil_mask) VALUES
('R',   'R',   1, 3, 1, '#5f86c2', '#a9c9f2', 'rgba(95,134,194,.40)',  '#101c38', '#080d1a', '#cfe0f7', 0.060, 'art'),
('SR',  'SR',  2, 4, 2, '#8a6fd0', '#cbb2ff', 'rgba(138,111,208,.46)', '#1a1440', '#0a0a1c', '#e2d6ff', 0.140, 'art'),
('SSR', 'SSR', 3, 5, 3, '#c9a24a', '#f6e3a8', 'rgba(201,162,74,.52)',  '#2a2148', '#0d0a18', '#f4d99a', 0.200, 'art'),
('UR',  'UR',  4, 5, 4, '#e0b45f', '#fff2cf', 'rgba(255,196,120,.62)', '#241a46', '#0b0817', '#ffeec2', 0.520, 'frame'),
('LR',  'LR',  5, 6, 5, '#f2f5ff', '#ffffff', 'rgba(180,205,255,.72)', '#1c2a52', '#070a15', '#ffffff', 0.720, 'frame');

-- 2) 卡定义。「系列」不新建词表，直接绑站点已有维度（section_kinds/site_dict）。
CREATE TABLE gacha_cards (
    id           bigserial PRIMARY KEY,
    key          text UNIQUE NOT NULL,
    name         text NOT NULL,
    name_i18n    jsonb NOT NULL DEFAULT '{}',
    rarity       text NOT NULL REFERENCES gacha_rarities(key),
    series_kind  text,
    series_value bigint,
    entity_table text,
    entity_ref   text,
    art_asset    text,
    synth_shards int NOT NULL DEFAULT 0,
    dupe_shards  int NOT NULL DEFAULT 0,
    lv_max       smallint NOT NULL DEFAULT 1 CHECK (lv_max >= 1),
    lv_cost_each int NOT NULL DEFAULT 0,
    lv_gain      numeric(4,3) NOT NULL DEFAULT 0,
    enabled      bool NOT NULL DEFAULT true,
    sort         int NOT NULL DEFAULT 0
);
CREATE INDEX gacha_cards_rarity_idx ON gacha_cards (rarity);

INSERT INTO gacha_cards (key, name, rarity, synth_shards, dupe_shards,
    lv_max, lv_cost_each, lv_gain, sort) VALUES
('demo-r',  '样例卡·R',  'R',   60,   8,   5,  12, 0.200, 1),
('demo-sr', '样例卡·SR', 'SR',  180,  20,  6,  22, 0.250, 2),
('demo-ssr','样例卡·SSR','SSR', 520,  60,  10, 40, 0.500, 3),
('demo-ur', '样例卡·UR', 'UR',  1400, 160, 10, 120, 0.500, 4),
('demo-lr', '样例卡·LR', 'LR',  3200, 400, 10, 400, 0.500, 5);

-- 3) 卡池（保底参数在池上；UP 卡可空）
CREATE TABLE gacha_banners (
    id           bigserial PRIMARY KEY,
    key          text UNIQUE NOT NULL,
    name         text NOT NULL,
    kind         text NOT NULL CHECK (kind IN ('standard','limited','newbie')),
    starts_at    timestamptz,
    ends_at      timestamptz,
    up_card_id   bigint REFERENCES gacha_cards(id),
    ticket_cost  int NOT NULL,
    ten_cost     int NOT NULL,
    daily_free   int NOT NULL DEFAULT 0,
    pity_soft    int NOT NULL,
    pity_hard    int NOT NULL CHECK (pity_hard >= pity_soft),
    pity_ramp    numeric(5,3) NOT NULL DEFAULT 0,
    pity_inherit bool NOT NULL DEFAULT true,
    pity_reset   bool NOT NULL DEFAULT true,
    guarantee_up bool NOT NULL DEFAULT false,
    up_ratio     numeric(4,3) NOT NULL DEFAULT 1,
    enabled      bool NOT NULL DEFAULT true
);

INSERT INTO gacha_banners (key, name, kind, ticket_cost, ten_cost,
    pity_soft, pity_hard, pity_ramp) VALUES
('demo-standard', '常驻样例池', 'standard', 25, 250, 50, 65, 5.000);

-- 4) 奖池行：三类产出一张表，判别列 + 复合 CHECK（不建并列固定列）
CREATE TABLE gacha_pool_rows (
    id          bigserial PRIMARY KEY,
    banner_id   bigint NOT NULL REFERENCES gacha_banners(id) ON DELETE CASCADE,
    output_type text NOT NULL CHECK (output_type IN ('card','shard','miss')),
    rarity      text REFERENCES gacha_rarities(key),
    card_id     bigint REFERENCES gacha_cards(id),
    weight      numeric(8,3) NOT NULL CHECK (weight >= 0),
    prize_value numeric(10,2) NOT NULL DEFAULT 0,
    shards      int NOT NULL DEFAULT 0,
    sort        int NOT NULL DEFAULT 0,
    CHECK ((output_type = 'card'  AND rarity IS NOT NULL)
        OR (output_type = 'shard' AND shards > 0)
        OR  output_type = 'miss')
);
CREATE INDEX gacha_pool_rows_banner_idx ON gacha_pool_rows (banner_id);

-- 种子七行 = 向量 demo_pool 逐字段同源（w/prize/shards 与卡列合成 RateRow）
INSERT INTO gacha_pool_rows (banner_id, output_type, rarity, card_id,
    weight, prize_value, shards, sort)
SELECT b.id, v.output_type, v.rarity, c.id, v.weight, v.prize, v.shards, v.sort
FROM gacha_banners b,
(VALUES
  ('miss',  NULL::text, NULL::text, 10.0,  0.0,    0, 1),
  ('shard', NULL,        NULL,       12.0,  0.0,   15, 2),
  ('card',  'R',   'demo-r',   50.0,  8.0,     0, 3),
  ('card',  'SR',  'demo-sr',  22.0,  20.0,    0, 4),
  ('card',  'SSR', 'demo-ssr',  2.0,  200.0,   0, 5),
  ('card',  'UR',  'demo-ur',   0.4,  800.0,   0, 6),
  ('card',  'LR',  'demo-lr',   0.1,  3000.0,  0, 7)
) AS v(output_type, rarity, card_key, weight, prize, shards, sort)
LEFT JOIN gacha_cards c ON c.key = v.card_key
WHERE b.key = 'demo-standard';
