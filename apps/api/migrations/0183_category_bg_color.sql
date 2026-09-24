-- 0183: 分类色收进 categories 表（bg_color），前端不再持有硬编码色表
--
-- 现状是两份互不相同的硬编码色表：`lib/format.tsx` 的 CATEGORY_COLORS 与
-- `components/torrent-table.tsx` 的 CAT_COLORS——同一种子卡片与详情头不同色；
-- 且两份都只覆盖 id 1..7，第 8 类起全落同一兜底色，站长加分类根本没有配色通道
-- （icon_key 有，颜色没有）。
-- 这里把颜色变成分类自身的一等属性：按位序铺一套 12 色（沿用列表页那套 catsprites
-- 色系，保证现有视觉不变），之后站长可按分类改。CHECK 锁死 #rrggbb，
-- 因为这个值最终会进内联 style。
-- 幂等：列已存在不重复加；只填 bg_color IS NULL 的行，站长改过的不动。

ALTER TABLE categories ADD COLUMN IF NOT EXISTS bg_color TEXT;

ALTER TABLE categories DROP CONSTRAINT IF EXISTS categories_bg_color_check;
ALTER TABLE categories ADD CONSTRAINT categories_bg_color_check
    CHECK (bg_color IS NULL OR bg_color ~ '^#[0-9a-fA-F]{6}$');

WITH ordered AS (
    SELECT id, row_number() OVER (ORDER BY id) AS rn FROM categories
),
palette(i, color) AS (
    VALUES (1, '#f6a5c0'), (2, '#7fb7e6'), (3, '#8fd6b5'), (4, '#f4d06f'),
           (5, '#b5a6f0'), (6, '#f6a07a'), (7, '#c9b8a3'), (8, '#8fc3e8'),
           (9, '#e8b48f'), (10, '#a8d8a0'), (11, '#d8a0c8'), (12, '#9fb4d8')
)
UPDATE categories c
SET bg_color = p.color
FROM ordered o
JOIN palette p ON p.i = ((o.rn - 1) % 12) + 1
WHERE c.id = o.id AND c.bg_color IS NULL;
