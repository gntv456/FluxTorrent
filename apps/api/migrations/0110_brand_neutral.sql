-- 0110: 通用建站系统 U2 —— 教育假设清扫（策划案 §7.4）
--
-- 默认品牌去教育化（教育包品牌保留「包子PT」）；存量站已改过 site_name 的不受影响
-- （仅当值仍是种子默认「包子PT」时才替换——站长自定义过就是有意的）。
-- 0037 的 site_type 默认 education 保持不变（T3：现状），仅品牌文案中性化。

UPDATE site_settings SET value = 'Flux 站点', updated_at = now()
WHERE name = 'site_name' AND value = '包子PT';

UPDATE site_settings SET value = '欢迎来到本站', updated_at = now()
WHERE name = 'announcement' AND value = '欢迎来到包子PT';

-- 教育包品牌保留原口径（apply 时仍写「包子PT」），其余包缺省走通用名
UPDATE site_type_packs SET brand = '' WHERE code NOT IN ('education') AND brand <> '';
