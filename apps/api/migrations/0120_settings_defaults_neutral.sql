-- 0120: 站点设定「基础信息 / SEO」默认值去教育化（通用 PT 建站系统）
--
-- 种子来源：0025（site_title/site_subtitle）、0034（SITENAME/titlekeywords/metakeywords/metadescription）
--   把这些键的默认值写成了教育站口径。全新安装若不处理，站长在「站点设定」会看到
--   「好学 FluxTorrent」「baozi」「教育,PT,种子」「教育资源私有种子社区」等非通用默认。
-- 仅当值仍是这些已知默认/教育口径时才替换；已自定义成其它值的站不受影响。

-- 全站标题 / 邮件署名 / RSS 频道名（NexusPHP 口径）
UPDATE site_settings SET value = 'FluxTorrent', updated_at = now()
WHERE name = 'SITENAME' AND value = '好学 FluxTorrent';

-- 站点副标题（种子默认是包子站昵称 'baozi'）
UPDATE site_settings SET value = '', updated_at = now()
WHERE name = 'site_title' AND value = 'baozi';

-- 站点口号（种子默认英文口号；另含历史遗留的教育口号）
UPDATE site_settings SET value = '', updated_at = now()
WHERE name = 'site_subtitle' AND value IN ('Share with love, Stay cozy', '好好学习天天向上');

-- SEO：标题关键词 / META 关键词 / META 描述
UPDATE site_settings SET value = 'PT,种子,资源', updated_at = now()
WHERE name = 'titlekeywords' AND value = '教育,种子,PT';

UPDATE site_settings SET value = 'PT,种子,资源', updated_at = now()
WHERE name = 'metakeywords' AND value = '教育,PT,种子';

UPDATE site_settings SET value = '私有种子社区', updated_at = now()
WHERE name = 'metadescription' AND value = '教育资源私有种子社区';
