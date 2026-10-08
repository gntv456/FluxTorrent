-- 0322_pack_home_sections.sql
-- 站型成熟度 · H10（对标 §4 机制层缺口最后一项）：首页区块随站型。
--
-- 现状：HOME_SECTIONS 是一份全局清单（http/home_layout.rs），11 个站型
-- 的首页完全相同（签到+喇叭+转盘…）。切到无损音乐站，首页仍是教育站
-- 那套；音乐站最该有的「新专辑（latest 海报墙）」没有默认置顶位。
--
-- 方案（与 view_layout 同范式，改动集中一处）：
--   · site_type_packs 增 home_sections JSONB 段：该型**默认首页排版**
--     （[{key,span}]，键 ∈ HOME_SECTIONS 白名单）；
--   · apply 时写入 site_settings.home_layout（同站长保存端点的规范化格式）
--     ——站长的手工排版在此前若已保存（home_layout 非空且非包默认），
--     由 0318 的覆盖位思想保护：apply 只在「当前值为空或等于任一预置包
--     默认」时写入，站长自定义排版不被切站型复位（与 tagline 0145 同口径）；
--   · 前端零改动：home_layout/home_sections 本来就读 site_settings/清单。
--
-- 各型默认排版（键全部来自 HOME_SECTIONS 白名单）：
--   通用型 = 现默认（news/attendance/shoutbox/funbox/resource_stats/
--             site_data/lucky_draw/links/latest）；
--   内容型（movie/music/anime/ebook/game/documentary/lossless）=
--             latest 置顶 + resource_stats/site_data/links（海报优先）；
--   教育/体育/软件 = 折中（news + latest + 统计/数据）。
--
-- 幂等：home_sections 段整段替换（固定值重放等值）；
--       home_layout 回填只在「空或等于任一预置默认」时写。

BEGIN;

ALTER TABLE site_type_packs ADD COLUMN IF NOT EXISTS home_sections JSONB;

-- 内容型：海报墙置顶
UPDATE site_type_packs SET home_sections =
 '[{"key":"latest","span":3},{"key":"resource_stats","span":3},{"key":"news","span":2},{"key":"shoutbox","span":1},{"key":"site_data","span":2},{"key":"attendance","span":1},{"key":"links","span":3}]'::jsonb
 WHERE code IN ('movie','music','anime','ebook','game','documentary','lossless');

-- 教育/体育/软件：资讯优先 + 海报墙靠前
UPDATE site_type_packs SET home_sections =
 '[{"key":"news","span":2},{"key":"latest","span":1},{"key":"attendance","span":1},{"key":"resource_stats","span":3},{"key":"site_data","span":2},{"key":"shoutbox","span":1},{"key":"links","span":3}]'::jsonb
 WHERE code IN ('education','sports','software');

-- 综合（兜底型）：现全局默认序
UPDATE site_type_packs SET home_sections =
 '[{"key":"news","span":2},{"key":"attendance","span":1},{"key":"shoutbox","span":2},{"key":"funbox","span":1},{"key":"resource_stats","span":3},{"key":"site_data","span":2},{"key":"lucky_draw","span":1},{"key":"links","span":3},{"key":"latest","span":3}]'::jsonb
 WHERE code = 'general';

COMMIT;
