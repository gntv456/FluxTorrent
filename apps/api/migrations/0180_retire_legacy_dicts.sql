-- 0180：旧三列词表退役（二审 R3 步骤一：止血 + 站型守卫）。
--
-- grades/media/editions 三张全局实体表是 0001 的教育硬编码（幼儿园…高三 /
-- 人教/部编/苏教…），任何站型共享。0174 的幂等回填还会把删掉的学段词
-- 「长回来」。本迁移：
-- 1) 三实体表按当前站型收敛：仅 education/ebook 保留学段/版本词表，
--    其余站型清空 grades/editions（media 保留通用媒介词）；
-- 2) section_dict 里从实体表灌入的教育词（grades/editions 维）同样按站型清；
-- 3) 0174 的回填语义在此之后不再触发（回填是幂等 INSERT…WHERE NOT EXISTS，
--    表空后每次启动重跑零行——但为绝后患，把 grades/editions 回填段改为
--    仅在表非空时执行没有意义；正确做法：表已清空即回填零行，天然安全）。
-- 守卫式：只清「仍为 0001 默认词」的行，站长手改/自建的词不动。
-- 存量种子引用不动（torrents.grade_id/edition_id 列与数据保留——展示侧
-- 0180 之前已由 site-profile 空词表自然隐藏）。

-- 1) 非教育站型：清 grades/editions 实体表默认教育词（保留站长自建行：
--   判据 = 名称在 0001 默认集内）
DO $$
DECLARE
  is_edu boolean;
BEGIN
  SELECT value IN ('education','ebook') INTO is_edu
    FROM site_settings WHERE name = 'site_type';
  IF NOT COALESCE(is_edu, false) THEN
    -- 存量种子引用的词保留（FK 保护；展示侧由 site-profile 空词表隐藏，
    -- 0180 只清「无引用的默认教育词」）
    DELETE FROM grades WHERE name = ANY(ARRAY[
      '幼儿园','一年级','二年级','三年级','四年级','五年级','六年级',
      '初一','初二','初三','高一','高二','高三'])
      AND NOT EXISTS (SELECT 1 FROM torrents t WHERE t.grade_id = grades.id);
    DELETE FROM editions WHERE name = ANY(ARRAY[
      '人教','部编','统编','苏教','北师大','外研','沪教'])
      AND NOT EXISTS (SELECT 1 FROM torrents t WHERE t.edition_id = editions.id);
    DELETE FROM section_dict WHERE kind = 'grades' AND name = ANY(ARRAY[
      '幼儿园','一年级','二年级','三年级','四年级','五年级','六年级',
      '初一','初二','初三','高一','高二','高三']);
    DELETE FROM section_dict WHERE kind = 'editions' AND name = ANY(ARRAY[
      '人教','部编','统编','苏教','北师大','外研','沪教']);
    -- media：'笔记/课件' 是教育专属媒介词，其余（视频/音频/书籍/文档/软件/图片）通用保留
    DELETE FROM media WHERE name IN ('笔记','课件')
      AND NOT EXISTS (SELECT 1 FROM torrents t WHERE t.medium_id = media.id);
    DELETE FROM section_dict WHERE kind = 'media' AND name IN ('笔记','课件');
  END IF;
END $$;

-- 2) education/ebook 站不动（词表本来就该在）；
--    未来切站型由 pack_apply 的 sections 重建接管——实体表只剩历史存档职责。
