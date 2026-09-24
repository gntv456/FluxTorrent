-- 0185：存量种子旧三列数据迁移进 sections 体系（二审 R3 第二步）。
--
-- torrents.medium_id/grade_id/edition_id 引用三张实体表；0087 起新数据走
-- torrent_sections（kind + dict_id）。0180 已清非教育站的旧词表（被引用行
-- 因 FK 保留）；本迁移把存量引用按名称对齐到 section_dict 现行词表：
--  - 教育系站（education/ebook/custom_*）：grades/editions 维度若存在，
--    按名称匹配补 torrent_sections 行；
--  - 全部站型：media 维按名称匹配补行（media 词表是通用的）。
-- 幂等：按 (torrent_id, kind) 唯一性，已存在的跳过。
-- 注意：旧三列本身不动（详情页老数据仍按实体表翻译，双轨展示不变）；
-- 本迁移只为「多维筛选能搜到老种子」补数据。

INSERT INTO torrent_sections (torrent_id, kind, dict_id)
SELECT t.id, 'media', sd.id
FROM torrents t
JOIN media m ON m.id = t.medium_id
JOIN section_dict sd ON sd.kind = 'media' AND sd.name = m.name
WHERE NOT EXISTS (
    SELECT 1 FROM torrent_sections ts
    WHERE ts.torrent_id = t.id AND ts.kind = 'media')
ON CONFLICT DO NOTHING;

-- 学段/版本仅教育系站迁移（其他站的 grades/editions 维度词表 0180 已清）
DO $$
DECLARE
  edu_like boolean;
BEGIN
  SELECT value IN ('education','ebook') OR value LIKE 'custom\_%' INTO edu_like
    FROM site_settings WHERE name = 'site_type';
  IF COALESCE(edu_like, false) THEN
    INSERT INTO torrent_sections (torrent_id, kind, dict_id)
    SELECT t.id, 'grades', sd.id
    FROM torrents t
    JOIN grades g ON g.id = t.grade_id
    JOIN section_dict sd ON sd.kind = 'grades' AND sd.name = g.name
    WHERE NOT EXISTS (
        SELECT 1 FROM torrent_sections ts
        WHERE ts.torrent_id = t.id AND ts.kind = 'grades')
    ON CONFLICT DO NOTHING;

    INSERT INTO torrent_sections (torrent_id, kind, dict_id)
    SELECT t.id, 'editions', sd.id
    FROM torrents t
    JOIN editions e ON e.id = t.edition_id
    JOIN section_dict sd ON sd.kind = 'editions' AND sd.name = e.name
    WHERE NOT EXISTS (
        SELECT 1 FROM torrent_sections ts
        WHERE ts.torrent_id = t.id AND ts.kind = 'editions')
    ON CONFLICT DO NOTHING;
  END IF;
END $$;
