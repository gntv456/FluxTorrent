-- 0422_section_modes_by_pack.sql
-- 分类模式回填：让发布表单只显示「本分类相关」的质量维度。
--
-- 病根（2026-10-26 实测）：/section-dict?category_id=N 的可见维度由
-- mode_kinds 决定，**该模式无 mode_kinds 行 = 全可见**（0195 兼容口径）。
-- 而站型包 apply 只写 section_kinds / section_dict，从不写 category_modes
-- ⇒ 库里 67 个分类 mode_id 全为 NULL、mode_kinds 空表 ⇒ 任何分类都把全站
-- 11 个站型的 42+ 维度铺进发布表单（一个 flex-wrap 行，观感即「乱」）。
--
-- 本迁移做四件事（全部幂等，重跑不重复）：
--   ① 词表消歧：「格式」重名（media=电子书词表 / format=音频词表）拆开：
--      media 收敛为「媒介」（影视类唯一语义），format 改名「音频格式」；
--   ② 补齐包内已引用、注册表却缺失的三个维度：codec / standard / team；
--   ③ 站型包一致性：media 只留在影视类包（其余包用各自专属维度），
--      避免重跑 apply 把 media 的词表/标签改回去；
--   ④ 按内容域建 13 个模式 + 按分类 key 绑定（key 是 0317 起的跨包稳定身份）。
--
-- 边界：只动「可见维度集合」，不清洗任何 torrent_sections 历史数据
--（全库仅 2 行，且都不在本次改动词表的 kind 上）。

BEGIN;

-- ============================================================
-- ① 词表消歧
-- ============================================================
-- format（FLAC/APE/...）→ 音频格式：与 media/bookformat 的「格式」区分开
UPDATE section_kinds SET label = '音频格式' WHERE kind = 'format';

-- media 收敛为「媒介」：原词表是电子书格式（ebook 包误占），由 bookformat
-- 取代（后者含 CBZ/CBR/DjVu/TXT，是超集），这里换成影视媒介词表
UPDATE section_kinds SET label = '媒介', sort = 10 WHERE kind = 'media';
DELETE FROM section_dict
 WHERE kind = 'media'
   AND name IN ('PDF', 'EPUB', 'MOBI/AZW3', '扫描版', '文字版');
INSERT INTO section_dict (kind, name, sort)
SELECT 'media', v.n, v.s FROM (VALUES
  ('原盘', 10), ('Blu-ray', 20), ('Remux', 30), ('重编码', 40),
  ('WEB-DL', 50), ('WEBRip', 60), ('HDTV', 70), ('DVD', 80)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
                    WHERE d.kind = 'media' AND d.name = v.n);

-- ============================================================
-- ② 补齐包内已引用的维度（movie/anime/documentary/sports/general/
--    lossless/software 包的 sections.kinds 都写了它们，注册表却没有）
-- ============================================================
INSERT INTO section_kinds
    (kind, label, sort, field_type, required, multiple, enabled)
VALUES
  ('codec',    '编码',   40, 'select', FALSE, FALSE, TRUE),
  ('standard', '分辨率', 60, 'select', FALSE, FALSE, TRUE),
  ('team',     '制作组', 90, 'text',   FALSE, FALSE, TRUE)
ON CONFLICT (kind) DO NOTHING;
-- 旧包把 standard 写成「规格」、codec 写成别的说法时，统一成本站口径
UPDATE section_kinds SET label = '编码'   WHERE kind = 'codec';
UPDATE section_kinds SET label = '分辨率' WHERE kind = 'standard';
UPDATE section_kinds SET label = '制作组' WHERE kind = 'team';

INSERT INTO section_dict (kind, name, sort)
SELECT 'codec', v.n, v.s FROM (VALUES
  ('H.264/x264', 10), ('H.265/x265', 20), ('AV1', 30), ('VP9', 40),
  ('MPEG-2', 50), ('VC-1', 60), ('Xvid', 70), ('其他', 90)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
                    WHERE d.kind = 'codec' AND d.name = v.n);

INSERT INTO section_dict (kind, name, sort)
SELECT 'standard', v.n, v.s FROM (VALUES
  ('2160p', 10), ('1080p', 20), ('1080i', 30), ('720p', 40),
  ('576p', 50), ('480p', 60), ('其他', 90)) AS v(n, s)
 WHERE NOT EXISTS (SELECT 1 FROM section_dict d
                    WHERE d.kind = 'standard' AND d.name = v.n);

-- ============================================================
-- ③ 站型包一致性（重跑 apply 不能把上面的口径改回去）
-- ============================================================
-- 3a. 非影视类包去掉 media：电子书用 bookformat、音乐用 format、
--     游戏用 platform、教育/软件各自有专属维度——media 在那些包里
--     分别被当成「格式/载体/平台」，语义互相打架
UPDATE site_type_packs p
   SET sections = jsonb_set(
         sections,
         '{kinds}',
         COALESCE((SELECT jsonb_agg(e)
                     FROM jsonb_array_elements(sections->'kinds') e
                    WHERE e->>'kind' <> 'media'), '[]'::jsonb),
         TRUE)
 WHERE p.code IN ('ebook', 'music', 'lossless', 'game', 'education',
                  'software')
   AND sections ? 'kinds';

UPDATE site_type_packs p
   SET sections = jsonb_set(
         sections,
         '{dict}',
         COALESCE(sections->'dict', '{}'::jsonb) - 'media',
         TRUE)
 WHERE p.code IN ('ebook', 'music', 'lossless', 'game', 'education',
                  'software');

-- 3b. 标签口径对齐（format → 音频格式、standard → 分辨率），
--     否则重跑 apply_pack_kinds 的 ON CONFLICT DO UPDATE 会把标签写回
UPDATE site_type_packs p
   SET sections = jsonb_set(
         sections,
         '{kinds}',
         (SELECT jsonb_agg(CASE
                   WHEN e->>'kind' = 'format'
                     THEN jsonb_set(e, '{label}', to_jsonb('音频格式'::text))
                   WHEN e->>'kind' = 'standard'
                     THEN jsonb_set(e, '{label}', to_jsonb('分辨率'::text))
                   ELSE e
                 END)
            FROM jsonb_array_elements(sections->'kinds') e),
         TRUE)
 WHERE sections ? 'kinds';

-- ============================================================
-- ④ 分类模式（一个内容域一个模式）+ 按分类 key 绑定
-- ============================================================
INSERT INTO category_modes (name)
SELECT v.n FROM (VALUES
  ('影视·电影'), ('影视·剧集'), ('纪录片'), ('动漫'), ('漫画'),
  ('音乐'), ('游戏'), ('软件'), ('电子书'), ('有声书'),
  ('教育'), ('体育'), ('其他')) AS v(n)
 WHERE NOT EXISTS (SELECT 1 FROM category_modes m WHERE m.name = v.n);

-- 4a. 各模式的可见维度（镜像 site_type_packs.sections.kinds，剔除
--     source —— 它的词表是电子书口径「正版/扫描/网络整理」，影视用
--     media 表达载体，混进来只会误导）
INSERT INTO mode_kinds (mode_id, kind, visible)
SELECT m.id, v.kind, TRUE
  FROM (VALUES
    ('影视·电影', 'media'), ('影视·电影', 'standard'),
    ('影视·电影', 'codec'), ('影视·电影', 'subtitle_group'),
    ('影视·电影', 'team'),

    ('影视·剧集', 'media'), ('影视·剧集', 'standard'),
    ('影视·剧集', 'codec'), ('影视·剧集', 'subtitle_group'),
    ('影视·剧集', 'team'), ('影视·剧集', 'season'),
    ('影视·剧集', 'episode_first'), ('影视·剧集', 'episode_last'),

    ('纪录片', 'media'), ('纪录片', 'standard'), ('纪录片', 'codec'),
    ('纪录片', 'network'), ('纪录片', 'doc_type'),
    ('纪录片', 'season'), ('纪录片', 'episode_first'),
    ('纪录片', 'episode_last'), ('纪录片', 'subtitle_group'),
    ('纪录片', 'team'), ('纪录片', 'narrator'), ('纪录片', 'award'),

    ('动漫', 'media'), ('动漫', 'standard'), ('动漫', 'codec'),
    ('动漫', 'studio'), ('动漫', 'subtitle_group'),
    ('动漫', 'mal_id'), ('动漫', 'bangumi_id'), ('动漫', 'anidb_id'),
    ('动漫', 'air_season'), ('动漫', 'season'),
    ('动漫', 'ep_first'), ('动漫', 'ep_last'), ('动漫', 'team'),

    ('漫画', 'author'), ('漫画', 'series'), ('漫画', 'bookformat'),
    ('漫画', 'dpi'), ('漫画', 'ocr'), ('漫画', 'source'),

    ('音乐', 'format'), ('音乐', 'encoding'), ('音乐', 'releasetype'),
    ('音乐', 'sample_rate'), ('音乐', 'bitdepth'), ('音乐', 'hi_res'),
    ('音乐', 'cue'), ('音乐', 'haslog'), ('音乐', 'log_score'),
    ('音乐', 'scene'), ('音乐', 'artist'), ('音乐', 'album'),

    ('游戏', 'platform'), ('游戏', 'gametype'), ('游戏', 'region'),
    ('游戏', 'drm_free'), ('游戏', 'dlc'), ('游戏', 'arch'),
    ('游戏', 'team'),

    ('软件', 'os'), ('软件', 'arch'), ('软件', 'version'),
    ('软件', 'license_type'), ('软件', 'team'),

    ('电子书', 'bookformat'), ('电子书', 'author'),
    ('电子书', 'series'), ('电子书', 'isbn'), ('电子书', 'dpi'),
    ('电子书', 'ocr'), ('电子书', 'source'), ('电子书', 'drm'),

    ('有声书', 'narrator'), ('有声书', 'author'),
    ('有声书', 'series'), ('有声书', 'isbn'),
    ('有声书', 'bookformat'),

    ('教育', 'subject'), ('教育', 'grade'),
    ('教育', 'resource_type'), ('教育', 'source'),
    ('教育', 'version'), ('教育', 'author'),

    ('体育', 'media'), ('体育', 'codec'), ('体育', 'league'),
    ('体育', 'season_label'), ('体育', 'round'), ('体育', 'team'),

    ('其他', 'version'), ('其他', 'region'),
    ('其他', 'studio'), ('其他', 'team')
  ) AS v(mode, kind)
  JOIN category_modes m ON m.name = v.mode
  JOIN section_kinds k ON k.kind = v.kind
ON CONFLICT (mode_id, kind) DO UPDATE SET visible = TRUE;

-- 4b. 分类 → 模式（按 key：0317 起跨包稳定身份，切站型/重跑 apply 不变）
UPDATE categories c
   SET mode_id = m.id
  FROM (VALUES
    ('general_movie', '影视·电影'), ('movie_bd', '影视·电影'),
    ('movie_remux', '影视·电影'), ('movie_webdl', '影视·电影'),
    ('movie_x264', '影视·电影'),

    ('general_tv', '影视·剧集'), ('movie_tv', '影视·剧集'),
    ('general_variety', '影视·剧集'), ('movie_variety', '影视·剧集'),

    ('general_doc', '纪录片'), ('movie_doc', '纪录片'),
    ('doc_nature', '纪录片'), ('doc_history', '纪录片'),
    ('doc_tech', '纪录片'), ('doc_military', '纪录片'),
    ('doc_humanity', '纪录片'), ('doc_adventure', '纪录片'),
    ('doc_food', '纪录片'),

    ('general_anime', '动漫'), ('anime_tv', '动漫'),
    ('anime_season', '动漫'), ('anime_movie', '动漫'),

    ('anime_manga', '漫画'), ('ebook_comic', '漫画'),

    ('general_music', '音乐'), ('anime_music', '音乐'),
    ('music_cn', '音乐'), ('music_west', '音乐'),
    ('music_jk', '音乐'), ('music_single', '音乐'),
    ('music_mv', '音乐'), ('music_live', '音乐'),
    ('music_lossless', '音乐'),
    ('lossless_album', '音乐'), ('lossless_hires', '音乐'),
    ('lossless_vinyl', '音乐'), ('lossless_sacd', '音乐'),
    ('lossless_sample', '音乐'), ('lossless_single', '音乐'),
    ('lossless_live', '音乐'),

    ('general_game', '游戏'), ('game_pc', '游戏'),
    ('game_console', '游戏'), ('game_handheld', '游戏'),
    ('game_cn', '游戏'), ('game_mod', '游戏'),
    ('game_ost', '游戏'), ('game_guide', '游戏'),

    ('general_software', '软件'),

    ('general_ebook', '电子书'), ('ebook_lit', '电子书'),
    ('ebook_social', '电子书'), ('ebook_tech', '电子书'),
    ('ebook_mag', '电子书'),

    ('ebook_audio', '有声书'),

    ('ebook_guide', '教育'),

    ('general_sports', '体育'), ('sport_football', '体育'),
    ('sport_basketball', '体育'), ('sport_tennis', '体育'),
    ('sport_fight', '体育'), ('sport_racing', '体育'),
    ('sport_mixed', '体育'), ('sport_highlights', '体育'),

    ('anime_goods', '其他'), ('anime_doujin', '其他')
  ) AS v(key, mode)
  JOIN category_modes m ON m.name = v.mode
 WHERE c.key = v.key;

-- 4c. 兜底：仍无模式的分类（无 key 的历史行/审计分类）归「其他」，
--     否则它继续走「无模式 = 全可见」分支，等于没修
UPDATE categories c
   SET mode_id = (SELECT id FROM category_modes WHERE name = '其他'
                   ORDER BY id LIMIT 1)
 WHERE c.mode_id IS NULL;

-- ============================================================
-- ⑤ 旧 7 列镜像（保留期兼容：管理面板读 mode_kinds，旧消费点读这些列）
-- ============================================================
UPDATE category_modes m SET
  show_source = EXISTS (SELECT 1 FROM mode_kinds k
                         WHERE k.mode_id = m.id AND k.visible
                           AND k.kind = 'source'),
  show_medium = EXISTS (SELECT 1 FROM mode_kinds k
                         WHERE k.mode_id = m.id AND k.visible
                           AND k.kind = 'media'),
  show_codec = EXISTS (SELECT 1 FROM mode_kinds k
                        WHERE k.mode_id = m.id AND k.visible
                          AND k.kind = 'codec'),
  show_audio_codec = EXISTS (SELECT 1 FROM mode_kinds k
                              WHERE k.mode_id = m.id AND k.visible
                                AND k.kind = 'audio_codec'),
  show_standard = EXISTS (SELECT 1 FROM mode_kinds k
                           WHERE k.mode_id = m.id AND k.visible
                             AND k.kind = 'standard'),
  show_processing = EXISTS (SELECT 1 FROM mode_kinds k
                             WHERE k.mode_id = m.id AND k.visible
                               AND k.kind = 'processing'),
  show_team = EXISTS (SELECT 1 FROM mode_kinds k
                       WHERE k.mode_id = m.id AND k.visible
                         AND k.kind = 'team')
 WHERE EXISTS (SELECT 1 FROM mode_kinds k WHERE k.mode_id = m.id);

COMMIT;
