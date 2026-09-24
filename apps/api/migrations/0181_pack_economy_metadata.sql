-- 0181：站型包 economy 预设补齐 + 元数据修正（二审 R10a）。
--
-- economy 预设此前仅 education 1/11；music/lossless 的 musicbrainz 源是
-- PT-Gen 白名单里的死字。本迁移：
-- 1) 11 包全部补 economy 预设（general 基准 + movie/sports 游戏化站点微调，
--    键集只动已登记 settings_meta 的经济键，apply_pack_extras 有白名单校验）；
-- 2) metadata：music/lossless 的 musicbrainz 移除（PT-Gen 不支持），
--    统一 douban；anime 补 bangumi 预设。
-- 幂等：直接 UPDATE 包行，重复执行同值。

-- general 基准（温和经济：游戏下注上限压低）
UPDATE site_type_packs SET economy = $$
{
  "games_max_bet": "500"
}
$$::jsonb WHERE code IN ('general','software','ebook','documentary','anime');

-- 影视/体育/游戏（内容驱动：下注放宽）
UPDATE site_type_packs SET economy = $$
{
  "games_max_bet": "1000"
}
$$::jsonb WHERE code IN ('movie','sports','game');

-- 音乐（轻度经济：下注收紧）
UPDATE site_type_packs SET economy = $$
{
  "games_max_bet": "300"
}
$$::jsonb WHERE code IN ('music','lossless');

-- education 保持既有 economy（0108 已有，不覆盖）

-- 元数据源修正
UPDATE site_type_packs SET metadata = '{"sources":["douban"]}'::jsonb
  WHERE code IN ('music','lossless');
UPDATE site_type_packs SET metadata = '{"sources":["bangumi","douban"]}'::jsonb
  WHERE code = 'anime';
