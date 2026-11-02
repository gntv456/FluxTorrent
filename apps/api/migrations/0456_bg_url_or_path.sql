-- 0456: 背景图三键改用 url_or_path 规则（允许站内相对路径）。
-- 编号让位：449 已按「official team rules」记进 _sqlx_migrations，同号会让
-- api 启动即 MigrateError::VersionMismatch(449)（干净安装则在第二次 apply 撞
-- 主键）——sqlx 只按版本号记账，同一号不可能落两个文件。
--
-- 0445/0447 只收 http(s) 绝对地址，于是站长只能把公网域名写进配置。实测
-- 这条路的代价很大：背景从本机访问要绕公网一圈，340 KB 的图 36 秒都拉不
-- 完（浏览器里等于没配）。相对路径让背景随访问入口同源加载，本机 / 局域网 /
-- 域名三种访问方式都不用改配置。
-- 引擎侧对应 settings_http/engine.rs 的 "url_or_path" 分支，前端侧对应
-- lib/site-background.ts（协议相对 //host 仍按外部地址处理）。

UPDATE settings_meta
   SET options = '{"rule":"url_or_path"}'::jsonb,
       hint = hint || '；也可填站内路径（如 /backgrounds/site-tide-photo.jpg），'
              || '随访问入口同源加载'
 WHERE name IN ('site_bg_url', 'login_bg_url', 'login_page_bg_url');
