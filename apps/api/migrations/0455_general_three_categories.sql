-- 0455_general_three_categories.sql
-- 综合站补三类：短剧 / 有声书 / 教育资源（站长 2026-11-02 提的产品口径）。
--
-- 现状：general（综合站）包只有 10 类（电影/电视剧/音乐/游戏/软件/动漫/
-- 纪录片/电子书/体育/综艺），对标当下内容供给缺三条——竖屏短剧已是独立
-- 内容形态、有声书在综合站没有入口（只在电子书包里）、教育资源在综合站
-- 也无处安放（教育包里的 edu_* 语义是 K12 学段，不是泛教育资源）。
--
-- 四条落点，缺一条就是半套（本仓反复抓的「同一件事两处口径」病形）：
--   ① 包载荷整段重写（13 类，key/sort/icon_key/bg_color/newznab_id 五件套，
--      与 0317/0325 同形）——决定「以后切到/重放综合站」的形态；
--   ② 物化进当前站点的 categories 行——决定「站长现在就能选到」，
--      不重放 apply 也得看见（沿用 0325 的「只补 NULL/只补缺行」纪律）；
--   ③ 绑分类模式（0422 的 mode_kinds 口径）——决定发布表单只露本域维度，
--      不绑就落回「该模式无行 = 全站 42+ 维度铺开」的老坑；
--   ④ 对外分类号：Newznab 侧新增 3020(Audio/AudioBook) 的名字映射
--      （代码侧 cat_map::newznab_name，与本迁移同批），并把电子书包里
--      「有声书」占用的 3030(Audio/MP3) 挪到 3020——同一概念在两个包给两个号，
--      Prowlarr/Jackett 侧就分家。
--
-- 号段口径（沿用 0267 的站内约定，站长后台可改）：
--   legacy_id 续 401..410 之后给 411/412/413；
--   newznab_id：短剧→5000(TV，与电视剧同父)、有声书→3020、教育资源→7020
--   （与 edu_* 一致，Newznab 标准树里没有教育类，归 Books 是本站既有选择）。
--
-- 幂等：包载荷整段替换（重放等值）；物化 ON CONFLICT (key) DO NOTHING；
-- 模式绑定只补 mode_id IS NULL 的行；legacy/newznab 只补空。

BEGIN;

-- ============ ① general 包载荷（13 类）============
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"general_movie","name":"电影","sort":10,"icon_key":"film","bg_color":"#3b82f6","newznab_id":2000},
 {"id":2,"key":"general_tv","name":"电视剧","sort":20,"icon_key":"tv","bg_color":"#8b5cf6","newznab_id":5000},
 {"id":3,"key":"general_shortdrama","name":"短剧","sort":25,"icon_key":"tv","bg_color":"#c084fc","newznab_id":5000},
 {"id":4,"key":"general_music","name":"音乐","sort":30,"icon_key":"music","bg_color":"#ec4899","newznab_id":3000},
 {"id":5,"key":"general_game","name":"游戏","sort":40,"icon_key":"game","bg_color":"#22c55e","newznab_id":4050},
 {"id":6,"key":"general_software","name":"软件","sort":50,"icon_key":"app","bg_color":"#14b8a6","newznab_id":4000},
 {"id":7,"key":"general_anime","name":"动漫","sort":60,"icon_key":"anime","bg_color":"#f97316","newznab_id":5070},
 {"id":8,"key":"general_doc","name":"纪录片","sort":70,"icon_key":"doc","bg_color":"#0ea5e9","newznab_id":5080},
 {"id":9,"key":"general_ebook","name":"电子书","sort":80,"icon_key":"book","bg_color":"#a3a615","newznab_id":7020},
 {"id":10,"key":"general_audiobook","name":"有声书","sort":85,"icon_key":"music","bg_color":"#f472b6","newznab_id":3020},
 {"id":11,"key":"general_sports","name":"体育","sort":90,"icon_key":"sport","bg_color":"#eab308","newznab_id":5060},
 {"id":12,"key":"general_variety","name":"综艺","sort":100,"icon_key":"tv","bg_color":"#d946ef","newznab_id":5090},
 {"id":13,"key":"general_edu","name":"教育资源","sort":110,"icon_key":"edu","bg_color":"#16a34a","newznab_id":7020}
]'::jsonb WHERE code = 'general';

-- ============ ①b 电子书包「有声书」对外号归一 3030 → 3020 ============
UPDATE site_type_packs SET categories = '[
 {"id":1,"key":"ebook_lit","name":"文学","sort":10,"icon_key":"book","bg_color":"#a3a615","newznab_id":7020},
 {"id":2,"key":"ebook_social","name":"社科","sort":20,"icon_key":"book","bg_color":"#84cc16","newznab_id":7020},
 {"id":3,"key":"ebook_tech","name":"科技","sort":30,"icon_key":"book","bg_color":"#65a30d","newznab_id":7020},
 {"id":4,"key":"ebook_mag","name":"杂志","sort":40,"icon_key":"book","bg_color":"#4d7c0f","newznab_id":7020},
 {"id":5,"key":"ebook_comic","name":"漫画","sort":50,"icon_key":"anime","bg_color":"#f97316","newznab_id":7020},
 {"id":6,"key":"ebook_audio","name":"有声书","sort":60,"icon_key":"music","bg_color":"#ec4899","newznab_id":3020},
 {"id":7,"key":"ebook_guide","name":"教辅","sort":70,"icon_key":"edu","bg_color":"#16a34a","newznab_id":7020}
]'::jsonb WHERE code = 'ebook';

-- 已物化的电子书行同步归一（只动仍是 3030 的那一条，站长改过的不覆盖）
UPDATE categories SET newznab_id = 3020
 WHERE key = 'ebook_audio' AND newznab_id = 3030;

-- ============ ② 物化进当前站点 ============
-- 只在这站点确实有综合站分类时补行（干净的教育站不该凭空多出 general_*）
WITH newcats(key, name, icon, srt, bg, nz, lg, seq) AS (
  VALUES
    ('general_shortdrama', '短剧',     'tv',    25, '#c084fc', 5000, 411, 1),
    ('general_audiobook',  '有声书',   'music', 85, '#f472b6', 3020, 412, 2),
    ('general_edu',        '教育资源', 'edu',  110, '#16a34a', 7020, 413, 3)
), base AS (
  SELECT COALESCE(max(id), 0) AS m FROM categories
), gated AS (
  SELECT n.* FROM newcats n
   WHERE (SELECT value FROM site_settings WHERE name = 'site_type') = 'general'
      OR EXISTS (SELECT 1 FROM categories c WHERE c.key LIKE 'general\_%')
)
INSERT INTO categories (id, key, name, icon_key, sort, bg_color,
                        newznab_id, legacy_id)
SELECT b.m + g.seq, g.key, g.name, g.icon, g.srt, g.bg, g.nz, g.lg
  FROM gated g, base b
 ON CONFLICT ((NULLIF(key, ''))) DO NOTHING;

-- 撞过 key 的旧行（站长手工建过同名分类）：只补空着的对外号
UPDATE categories c
   SET newznab_id = COALESCE(c.newznab_id, v.nz),
       legacy_id  = COALESCE(c.legacy_id, v.lg)
  FROM (VALUES ('general_shortdrama', 5000, 411),
               ('general_audiobook',  3020, 412),
               ('general_edu',        7020, 413)) AS v(key, nz, lg)
 WHERE c.key = v.key;

-- ============ ③ 绑分类模式（0422 口径：只补没绑过的）============
UPDATE categories c
   SET mode_id = m.id
  FROM (VALUES ('general_shortdrama', '影视·剧集'),
               ('general_audiobook',  '有声书'),
               ('general_edu',        '教育')) AS v(key, mode)
  JOIN category_modes m ON m.name = v.mode
 WHERE c.key = v.key AND c.mode_id IS NULL;

COMMIT;
