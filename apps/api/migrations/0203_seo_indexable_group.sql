-- 0203：把 0201 新加的 `seo_indexable` 归到 SEO 那三个键所在的分区。
--
-- 可见层复验抓到的：`metadescription`/`metakeywords`/`analyticscode` 的
-- `site_settings.grp` 一直是 `tweak`（次要设定），而 0201 插值行时写了 `main`
-- （主要设定）。四个键的 `settings_meta.group_key` 同为「SEO 与统计」，结果这张
-- 卡片在两个分区里各出现一次，「允许收录」孤零零一个字段——站长在同一张卡里
-- 找不到描述与关键词。
--
-- 动新键不动老键：收录开关去join它那三个邻居，而不是把三个用了多年的键搬走。
UPDATE site_settings
SET grp = 'tweak'
WHERE name = 'seo_indexable'
  AND grp <> 'tweak';
