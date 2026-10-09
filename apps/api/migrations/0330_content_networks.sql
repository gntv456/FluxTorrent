-- 0330_content_networks.sql
-- 站型成熟度 · 出品方聚合页（documentary 专项 P1 深水区，对标 §7.1）：
--
-- 纪录片最核心的检索入口不是"题材"而是"哪家出品"——BBC/NHK/PBS/国家地理
-- 是用户选片的天然锚点，成熟纪录片站（如纪录片之家、BBC Earth 档案站）
-- 都有厂牌页。0329 已把 `network` 建成 multiselect 维度，但**维度只能筛
-- 不能聚合**：点不出"BBC 的全部纪录片"这一页。
--
-- 承载评估（延续 0327 artists 的「低基数实体 + sections 不动」范式）：
--   · 与 artists 同量级低基数，值仍存 torrent_sections.kind='network'
--     （dict_id 指向 section_dict.name），维度链路完全不动；
--   · 本表只做**按名聚合的锚点**：/networks 列表 + /networks/{id} 厂牌页
--     + 厂牌榜，读口按 section_dict.name 反查 sections（与 artists 同法）。
--
-- 与 artists 的差别：network 是 **select 枚举**（值在 section_dict，走
-- dict_id），artist 是 **text 自由值**（走 value）。所以本表建行时以
-- `section_dict.name` 为源，而不是 jsonb 数组拆文本。
--
-- 通用性说明（纪律：不做单站硬编码）：表名用中性 `content_networks`
-- 而非 `networks`，语义=「内容出品/发行实体」，可同时承载：
--   · documentary.network（出品方）
--   · movie/general 将来的 studio（制片厂）
--   · anime 将来的 animation_studio（动画制作）
-- 维度名不同、实体同源 ⇒ 一张表按 `kind` 列区分来源维度，避免每站建表。
--
-- 幂等：CREATE IF NOT EXISTS + 回填 ON CONFLICT DO NOTHING。

BEGIN;

CREATE TABLE IF NOT EXISTS content_networks (
    id BIGSERIAL PRIMARY KEY,
    -- 实体来源维度：documentary=network，movie=studio，anime=studio …
    kind TEXT NOT NULL DEFAULT 'network',
    name TEXT NOT NULL,
    -- 归一化键（去空白小写）防同义重复
    norm_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (kind, name)
);

CREATE INDEX IF NOT EXISTS idx_content_networks_kind
    ON content_networks (kind, norm_name);

-- 回填：现存 section_dict 里 network 维度的全部词条建行。
-- （documentary 包 apply 后 network 词表已在 section_dict；此处按维度建锚点）
INSERT INTO content_networks (kind, name, norm_name)
SELECT DISTINCT sd.kind,
                TRIM(sd.name),
                LOWER(REGEXP_REPLACE(TRIM(sd.name), '\s+', '', 'g'))
FROM section_dict sd
JOIN section_kinds sk ON sk.kind = sd.kind
WHERE sk.field_type IN ('select', 'multiselect')
  AND TRIM(sd.name) <> ''
ON CONFLICT (kind, name) DO NOTHING;

COMMIT;
