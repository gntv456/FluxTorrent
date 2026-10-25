-- 0398_forum_hot_views.sql
-- 论坛话题行「热 N」热度标阈值（管理端可配）。
--
-- 背景：样图话题行有 `热 1.2k`（浏览过千才显示），此前阈值硬编码在
-- 前端 components/forum-home-stream.tsx 的 hotTag()（views >= 1000）。
-- 小站浏览量天然低，阈值该由站长按站点体量调——做成 settings 项：
-- 管理端「论坛」组出现一个数字输入框，0 = 关闭热度标。
--
-- 两条一起插：settings_meta（管理端 schema 的唯一来源，没 meta 行就
-- 不进设置页）＋ site_settings（当前值，缺行时后端按默认 1000 兜底）。
--
-- ⚠️ 顺序不能反：settings_meta.name 对 site_settings.name 有外键
-- （首版写反过，sqlx 报 settings_meta_name_fkey 违规，api 崩循环）。

INSERT INTO site_settings (name, value, descr, grp)
VALUES (
    'forum_hot_views', '1000',
    '话题行热度标阈值（浏览数，0=关闭）', 'forum'
)
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (
    name, type, label_zh, label_en, hint, unit,
    min, max, step, group_key, card_order, min_class, visible
) VALUES (
    'forum_hot_views', 'number', '话题热度标阈值',
    'Hot tag view threshold',
    '浏览数达到该值，话题行末尾显示「热 N」；0 = 关闭热度标。',
    '次浏览', 0, 1000000, 100, '话题展示', 2, 93, true
)
ON CONFLICT (name) DO NOTHING;
