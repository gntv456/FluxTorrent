-- 0119: 新站默认模板改为「综合站(general)」
-- 项目是通用 PT 建站系统，默认站型不应绑定教育站（包子站口径）。
--
-- 背景：0001 种子把 categories 写成教育集、0037 把默认 site_type 写成 education、
--       0039 把 module_textbooks 默认写成 yes —— 这三处共同构成"默认即教育站"。
--       迁移只执行一次，因此本迁移把「初装默认态」一次性翻转为 general；
--       之后站长在向导里主动选 education 会被记录，不会再次被本迁移覆盖。
--
-- 1) 默认站型 education → general
UPDATE site_settings SET value = 'general', updated_at = now()
WHERE name = 'site_type' AND value = 'education';

-- 2) 教育专属模块（课本中心）默认关闭：通用模板不含 textbooks
UPDATE site_settings SET value = 'no', updated_at = now()
WHERE name = 'module_textbooks' AND value = 'yes';

-- 3) 默认分类：教育集 → general 包快照（与 site_type_packs.general.categories 一致）
--    仅当分类仍是教育站默认集（id=1 学前教育 且 id=7 纪录片）时处理；
--    无 torrent 引用则整体重建，有引用则原地改名保 id（维持 torrents.category_id 外键）。
DO $$
DECLARE
    is_edu_default boolean;
    refs bigint;
BEGIN
    SELECT (
        EXISTS(SELECT 1 FROM categories WHERE id = 1 AND name = '学前教育')
        AND EXISTS(SELECT 1 FROM categories WHERE id = 7 AND name = '纪录片')
    ) INTO is_edu_default;
    IF NOT is_edu_default THEN
        RAISE NOTICE '0119: 分类非教育站默认集，跳过（视为已自定义）';
        RETURN;
    END IF;

    SELECT count(*) INTO refs FROM torrents;
    IF refs = 0 THEN
        DELETE FROM categories;
        INSERT INTO categories (id, name) VALUES
            (1,'电影'),(2,'电视剧'),(3,'音乐'),(4,'游戏'),(5,'软件'),
            (6,'动漫'),(7,'纪录片'),(8,'电子书'),(9,'体育'),(10,'综艺');
        RAISE NOTICE '0119: 分类已重建为 general 10 类（无引用）';
    ELSE
        UPDATE categories SET name = '电影'   WHERE id = 1;
        UPDATE categories SET name = '电视剧' WHERE id = 2;
        UPDATE categories SET name = '音乐'   WHERE id = 3;
        UPDATE categories SET name = '游戏'   WHERE id = 4;
        UPDATE categories SET name = '软件'   WHERE id = 5;
        UPDATE categories SET name = '动漫'   WHERE id = 6;
        UPDATE categories SET name = '纪录片' WHERE id = 7;
        INSERT INTO categories (id, name) VALUES (8,'电子书'),(9,'体育'),(10,'综艺')
            ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;
        RAISE NOTICE '0119: 分类已原地改名保 id 为 general 10 类（存在 % 条 torrent 引用）', refs;
    END IF;
END $$;
