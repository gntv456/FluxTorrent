-- 0201：SEO 面板收口（四审 L6「SEO 面板全断」）
--
-- 本批起真正消费：metadescription（meta description + OG 描述，RSS 早已用它，
-- 现在前台与 RSS 同一份）、metakeywords（meta keywords）、新增 seo_indexable
-- （决定 robots.txt 是否放开 + 页面是否带 noindex）。
--
-- 摘除两个接不上的键（纪律：要么真生效要么删，不留中间态）：
--   titlekeywords —— 在本栈里没有对应的渲染位（标题由 site_name + 字典后缀组成），
--                    硬塞进去只会变成第二个「填了没用的框」；
--   cssdate       —— Next 自带产物内容哈希版本化，手填 CSS 版本日期不起作用。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('seo_indexable', 'no', '允许搜索引擎收录（robots 与 meta robots）', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order)
SELECT 'seo_indexable', 'yesno', '允许收录', 'Allow indexing',
       '关闭时 robots.txt 全站 Disallow、页面带 noindex；私有站建议保持关闭',
       'SEO 与统计',
       COALESCE((SELECT max(card_order) + 1 FROM settings_meta m
                 WHERE m.group_key = 'SEO 与统计'), 90)
WHERE NOT EXISTS (SELECT 1 FROM settings_meta s WHERE s.name = 'seo_indexable');

DELETE FROM settings_meta WHERE name IN ('titlekeywords', 'cssdate');
DELETE FROM site_settings WHERE name IN ('titlekeywords', 'cssdate');
