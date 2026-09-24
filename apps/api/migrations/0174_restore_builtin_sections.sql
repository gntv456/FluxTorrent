-- 0174: 恢复被「应用站型包」清掉的质量维度（内置九维 + 三套字典回填）
--
-- 现场成因：pack_apply 的 0101 清理逻辑会把「内置九维中未被本包定义」的维度
-- 整体 DELETE，而 site_type_packs 里 general / education / anime / documentary /
-- software / sports / lossless 共 8 个包的 sections 列为 NULL（即本包不管维度）。
-- 应用这些包时九维全部命中「未被定义」，in_use 守卫在新站/测试站又不成立
-- （torrent_sections 为空），于是 section_kinds → section_dict → torrent_sections
-- 经 ON DELETE CASCADE 被连锁清空，发布页「质量」与高级搜索「多维筛选」整体失效，
-- 前端因此只能退回硬编码词表（分类/学段/媒介显示 bug 的土壤）。
-- 代码侧同批修复：apps/api/src/staff_http/pack_apply.rs —— 包未声明 sections
-- 时不动任何维度。本迁移负责把已被删掉的数据灌回来，全程幂等可重复执行。

-- 1) 内置九维（与 0085 的种子逐字一致）
INSERT INTO section_kinds (kind, label, sort) VALUES
  ('media',       '媒介',     10),
  ('grades',      '学段',     20),
  ('editions',    '版本',     30),
  ('codec',       '编码',     40),
  ('audio_codec', '音频编码', 50),
  ('standard',    '规格',     60),
  ('source',      '来源',     70),
  ('processing',  '处理工艺', 80),
  ('team',        '制作组',   90)
ON CONFLICT (kind) DO NOTHING;

-- 2) 三套字典行从实体表回填（0088 口径：实体表保留作历史存档，sort 沿用旧 id
--    ——实体表自身的 sort 列全是 0，拿它排序会退化成无序）
INSERT INTO section_dict (kind, name, sort)
SELECT 'media', m.name, m.id FROM media m
WHERE NOT EXISTS (
    SELECT 1 FROM section_dict sd WHERE sd.kind = 'media' AND sd.name = m.name
);

INSERT INTO section_dict (kind, name, sort)
SELECT 'grades', g.name, g.id FROM grades g
WHERE NOT EXISTS (
    SELECT 1 FROM section_dict sd WHERE sd.kind = 'grades' AND sd.name = g.name
);

INSERT INTO section_dict (kind, name, sort)
SELECT 'editions', e.name, e.id FROM editions e
WHERE NOT EXISTS (
    SELECT 1 FROM section_dict sd
    WHERE sd.kind = 'editions' AND sd.name = e.name
);

-- 3) 存量种子的三列引用 → torrent_sections（照 0088：按名称映射，
--    DISTINCT ON 取最小 dict_id 防重，已有 sections 的行不重复插）
INSERT INTO torrent_sections (torrent_id, kind, dict_id)
SELECT DISTINCT ON (t.id) t.id, 'media', sd.id
FROM torrents t
JOIN media m ON m.id = t.medium_id
JOIN section_dict sd ON sd.kind = 'media' AND sd.name = m.name
WHERE t.medium_id IS NOT NULL
ON CONFLICT DO NOTHING;

INSERT INTO torrent_sections (torrent_id, kind, dict_id)
SELECT DISTINCT ON (t.id) t.id, 'grades', sd.id
FROM torrents t
JOIN grades g ON g.id = t.grade_id
JOIN section_dict sd ON sd.kind = 'grades' AND sd.name = g.name
WHERE t.grade_id IS NOT NULL
ON CONFLICT DO NOTHING;

INSERT INTO torrent_sections (torrent_id, kind, dict_id)
SELECT DISTINCT ON (t.id) t.id, 'editions', sd.id
FROM torrents t
JOIN editions e ON e.id = t.edition_id
JOIN section_dict sd ON sd.kind = 'editions' AND sd.name = e.name
WHERE t.edition_id IS NOT NULL
ON CONFLICT DO NOTHING;
