-- 0117: 去包子化（「包子PT」是真实站点名，不应作为通用 PT 建站系统的默认/兜底品牌）
--
-- 根因：默认站型 site_type=education，education 包 brand 写死「包子PT」；
--       site-profile 接口优先取 site_name，取不到（null/空）才回落到当前站型包的 brand。
--       迁移 0110 当时用 WHERE code NOT IN ('education') 把 education 的「包子PT」刻意保留，
--       导致 site_name 一旦被重置/回滚/重灌，包子PT 就从 education 包兜底冒出。
-- 处置：把 education 包品牌从真实站名「包子PT」改为通用占位「教育站」
--       （与其他包一致：影站/乐站/漫站/书站/综合站/体站/游站/软站/纪录站/无损站），
--       并兜底把仍是包子文案的 site_name / announcement 中性化。
-- 已自定义过品牌的站不受影响（精确匹配，不误伤）。

UPDATE site_type_packs SET brand = '教育站' WHERE code = 'education' AND brand = '包子PT';

UPDATE site_settings SET value = 'Flux 站点', updated_at = now()
WHERE name = 'site_name' AND value = '包子PT';

UPDATE site_settings SET value = '欢迎来到本站', updated_at = now()
WHERE name = 'announcement' AND value = '欢迎来到包子PT';
