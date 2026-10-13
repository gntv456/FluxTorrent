-- 0358 TMDB 元数据第五源（直连路线，不走 WASM/PT-Gen 上游）。
-- 依据：ptgen.rs 此前只认 imdb/douban/bangumi/indienova 四源 host；
-- TMDB 是影视站事实标准条目库（poster/overview/external_ids 一发拿全），
-- 且官方 API 无需 OAuth，v3 api_key 直连即可。fetch_tmdb 按 URL 里的
-- /movie/{id} 或 /tv/{id} 取数，折成 PT-Gen 同形三元组
-- {name, descr, poster}，发种页/自动补全无感接入。
--
-- 口径：metadata_sources 追加 ',tmdb' token（照 0221 mediainfo 先例）；
-- tmdb_api_key 默认空 = 禁用（runtime 判空给「未配置」文案，不是 500）。
-- 幂等（ON CONFLICT DO NOTHING + position 判重），重跑安全。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('tmdb_api_key', '',
        'TMDB API 密钥（v3 api_key）：配置后发布页 PT-Gen 支持 '
        'themoviedb.org 链接直连抓取（标题/简介/海报）。留空 = 该源禁用。',
        'security')
ON CONFLICT (name) DO NOTHING;

-- 默认串追加 tmdb token：只对「未含 tmdb」的现有值追加（幂等，不重复叠）
UPDATE site_settings
SET value = value || ',tmdb'
WHERE name = 'metadata_sources'
  AND position(',tmdb' IN ',' || value || ',') = 0;

-- settings_meta 登记（照 0227 captcha_secret 先例：type password +
-- secret=true，GET 回掩码；安全组——和上游密钥同类）
INSERT INTO settings_meta
  (name, type, secret, label_zh, label_en, hint, group_key, card_order)
VALUES ('tmdb_api_key', 'password', true,
        'TMDB API 密钥', 'TMDB API key',
        'themoviedb.org 官方 v3 api_key（console 申请）；配置后发布页 '
        'PT-Gen 支持 TMDB 条目链接。留空 = 禁用该源。',
        'security', 10)
ON CONFLICT (name) DO NOTHING;

-- metadata_sources 的 settings_meta 提示语补 tmdb（条件更新，重跑安全）
UPDATE settings_meta
SET hint = '逗号分隔：imdb / douban / bangumi / indienova / tmdb '
           '（音乐/图书站可只留 douban 或留空禁用 PT-Gen）'
WHERE name = 'metadata_sources'
  AND hint LIKE '%indienova%'
  AND position('tmdb' IN hint) = 0;
