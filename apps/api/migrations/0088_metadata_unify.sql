-- 0088: 元数据通用化 P0（原号 0087 与 p2_hardening 撞号，让位改 88） —— media/grades/editions 并入 section_dict + 元数据源可配置
--
-- 1) torrents.medium_id 放开 NOT NULL：新站型不再被教育站三实体表绑架；
--    存量列保留为兼容口径（老数据展示/筛选照旧），新数据以 torrent_sections 为准。
-- 2) 三张实体表的字典行迁入 section_dict（kind = media/grades/editions），
--    存量种子的三列引用按「名称」映射补写 torrent_sections。
-- 3) 新增站点设置 metadata_sources：该站启用的元数据源（csv），
--    控制上传页 IMDb 输入框显隐与 PT-Gen 白名单。

ALTER TABLE torrents ALTER COLUMN medium_id DROP NOT NULL;

-- ---- 1) 实体表字典行迁入 section_dict（实体表保留，仅作历史口径存档） ----
INSERT INTO section_dict (kind, name, sort)
SELECT 'media', name, id FROM media
WHERE NOT EXISTS (SELECT 1 FROM section_dict sd WHERE sd.kind = 'media' AND sd.name = media.name);

INSERT INTO section_dict (kind, name, sort)
SELECT 'grades', name, id FROM grades
WHERE NOT EXISTS (SELECT 1 FROM section_dict sd WHERE sd.kind = 'grades' AND sd.name = grades.name);

INSERT INTO section_dict (kind, name, sort)
SELECT 'editions', name, id FROM editions
WHERE NOT EXISTS (SELECT 1 FROM section_dict sd WHERE sd.kind = 'editions' AND sd.name = editions.name);

-- ---- 2) 存量种子的三列引用 → torrent_sections（按名称映射，取最小 dict_id 防重） ----
INSERT INTO torrent_sections (torrent_id, kind, dict_id)
SELECT DISTINCT ON (t.id) t.id, 'media', sd.id
FROM torrents t
JOIN media m ON m.id = t.medium_id
JOIN section_dict sd ON sd.kind = 'media' AND sd.name = m.name
ON CONFLICT DO NOTHING;

INSERT INTO torrent_sections (torrent_id, kind, dict_id)
SELECT DISTINCT ON (t.id) t.id, 'grades', sd.id
FROM torrents t
JOIN grades g ON g.id = t.grade_id
JOIN section_dict sd ON sd.kind = 'grades' AND sd.name = g.name
ON CONFLICT DO NOTHING;

INSERT INTO torrent_sections (torrent_id, kind, dict_id)
SELECT DISTINCT ON (t.id) t.id, 'editions', sd.id
FROM torrents t
JOIN editions e ON e.id = t.edition_id
JOIN section_dict sd ON sd.kind = 'editions' AND sd.name = e.name
ON CONFLICT DO NOTHING;

-- ---- 3) 元数据源设置（csv；默认与既有行为一致） ----
INSERT INTO site_settings (name, value, descr, grp) VALUES
('metadata_sources', 'imdb,douban,bangumi,indienova', '启用的元数据源（csv）：imdb/douban/bangumi/indienova；控制上传页条目链接输入与 PT-Gen 范围', 'basic')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class) VALUES
('metadata_sources', 'text', '元数据源', 'Metadata sources', '逗号分隔：imdb / douban / bangumi / indienova（音乐/图书站可只留 douban 或留空禁用 PT-Gen）', '基础信息', 10, 99)
ON CONFLICT (name) DO UPDATE SET
    label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
    hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
    card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class;
