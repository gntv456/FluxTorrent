-- 0221：G10 —— metadata_sources 默认纳入 mediainfo（MediaInfo 录入/展示默认开）
--
-- 0208 把 'mediainfo' 定为 metadata_sources 的开关 token（不发新键，运行时按数组
-- 判断）：在列里 ⇒ 上传/编辑页显示 MediaInfo 文本框、详情页渲染该块；不在列里
-- ⇒ 两处都隐藏。但 0088 的默认值与站型包预设都没有这个 token，于是新装与升级
-- 站点默认看不到 MediaInfo——站长得先知道存在这个 token 才能开启。
-- MediaInfo 是纯文本内容块（不依赖任何外部服务，与 imdb/douban 那类「外部源」
-- 性质不同），通用建站下默认可达更合理；站长把 token 从列里删掉即可关闭。
--
-- 三步同改：设置值（存量+新装）/ 站型包预设（应用包会覆盖设置值）/ 设置卡提示。

-- 1) 设置值：非空列表补 mediainfo（留空 = 站长显式全关，保持不动）
UPDATE site_settings
SET value = rtrim(btrim(value), ',') || ',mediainfo'
WHERE name = 'metadata_sources'
  AND btrim(value) <> ''
  AND value NOT LIKE '%mediainfo%';

-- 2) 站型包预设：sources 数组补 mediainfo（数组里已有则不动）
UPDATE site_type_packs
SET metadata = jsonb_set(
        metadata, '{sources}',
        (metadata -> 'sources') || '["mediainfo"]'::jsonb)
WHERE metadata ? 'sources'
  AND NOT (metadata -> 'sources' ? 'mediainfo');

-- 3) 设置卡提示：token 清单补 mediainfo 与「留空=全关」口径
UPDATE settings_meta
SET hint = '逗号分隔：imdb / douban / bangumi / indienova / mediainfo'
    || '（MediaInfo 为纯文本块）；留空 = 全关（不显示条目链接输入、'
    || '禁用 PT-Gen、隐藏 MediaInfo）'
WHERE name = 'metadata_sources';
