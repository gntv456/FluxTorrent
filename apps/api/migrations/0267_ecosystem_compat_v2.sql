-- 0267 第三方工具对接 v2（2026-10-02）：对外契约补齐。
--   ① categories.newznab_id：Torznab/媒体库生态的分类映射（此前对外一律 8000 Other）。
--   ② categories.legacy_id 回填：NexusPHP 4xx 口径分类 id（此前全 NULL，
--      期望 NP 分类树的工具拿到 1..10 会错位）。
--   ③ admin_audit / nothing else —— 本批其余改动均为代码侧，无新表。
--
-- 口径说明（站长可自行调整，不写死到代码）：
--   legacy_id  = NexusPHP 生态惯用的 4xx 分类号（401 电影 … 410 电子书）
--   newznab_id = Newznab/Torznab 标准分类号（2000 Movies / 5000 TV / …）
-- 二者都按「内置十分类名」做一次性回填，只覆盖仍为空的行 —— 站长已手工
-- 调整过的分类不会被本迁移覆盖（幂等）。

ALTER TABLE categories ADD COLUMN IF NOT EXISTS newznab_id integer;

-- 内置十分类：name → (legacy_id, newznab_id)
-- 用 VALUES 联结按名回填；同名重复时只改仍为 NULL 的行。
WITH mapping(name, np_id, nz_id) AS (
    VALUES
        ('电影',   401, 2000),
        ('电视剧', 402, 5000),
        ('综艺',   403, 5090),
        ('音乐',   404, 3000),
        ('动漫',   405, 5070),
        ('体育',   406, 5060),
        ('纪录片', 407, 5080),
        ('软件',   408, 4000),
        ('游戏',   409, 4050),
        ('电子书', 410, 7020)
)
UPDATE categories c
   SET legacy_id  = COALESCE(c.legacy_id, m.np_id),
       newznab_id = COALESCE(c.newznab_id, m.nz_id)
  FROM mapping m
 WHERE c.name = m.name;

-- 兜底：本迁移前站长自建、名字不在映射表里的分类 → newznab_id 落 8000(Other)，
-- 保证对外层永远有值可发（不能是 NULL，否则 Torznab item 分类缺失）。
UPDATE categories SET newznab_id = 8000 WHERE newznab_id IS NULL;

-- 对外层按 newznab_id/legacy_id 反查，建普通索引（分类是小表，仅避免全表扫）
CREATE INDEX IF NOT EXISTS idx_categories_newznab ON categories(newznab_id);
