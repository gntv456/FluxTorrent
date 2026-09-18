-- 0118: 默认品牌统一为 FluxTorrent
-- 项目是通用 PT 建站系统，初始默认状态不绑定任何具体站名 / 类型占位品牌。
--
-- 背景：site-profile 接口优先取 site_name，取不到才回落到当前站型包的 brand。
--       建站向导允许留空站名（setup_http.rs:141），apply_pack 端点会把 site_name 写成包 brand（http.rs:4567）。
--       所以要让「默认激活品牌 = FluxTorrent」，必须同时处理两处来源。
--
-- 1) site_name：仅当仍是已知占位值（包子PT / Flux 站点 / 空）时改为 FluxTorrent；
--    已自定义站点名的站不受影响（不误伤）。
UPDATE site_settings SET value = 'FluxTorrent', updated_at = now()
WHERE name = 'site_name'
  AND (value = '包子PT' OR value = 'Flux 站点' OR value = '' OR value IS NULL);

-- 2) 清空各站型包的 brand 占位：使「未设站点名」时回落到 FluxTorrent
--    （前端 layout.tsx 硬编码兜底 profile.brand || "FluxTorrent"），而不是 教育站/影站 等类型占位。
--    已设站点名（site_name 非空）的站由 site_name 优先，不受此影响；
--    apply_pack 写空 brand 也只会把站点名置空、最终回落 FluxTorrent，符合默认品牌预期。
UPDATE site_type_packs SET brand = '' WHERE brand <> '';
