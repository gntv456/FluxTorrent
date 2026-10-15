-- 0375 RSS/Torznab 存活口径开关（批一百三十二留待站方裁的那条）
--
-- 批一百三十二实测发现：主列表走 0102 的存活三态（默认只给活种），
-- 而 RSS/Torznab 的 WHERE 里**没有**那份谓词 ⇒ 恒按「全部」给。
-- 后果是刷流器会抓到 0 源种并卡住。
--
-- 为什么不直接翻转 RSS：刷流器（qBittorrent 等）的既有订阅已经按
-- 「全部」在跑，直接翻转会让它们突然少掉源种。也不该在 RSS 里另写
-- 一份 seeders 谓词——那会与主列表的口径漂移，正是审计里反复出现
-- 的病根。
--
-- 口径：站内开关，默认 no（保持今天的「全部」行为，零破坏）；
-- 站长要「刷流只要活种」时改成 yes，改法是把**同一份**谓词接进 RSS。
-- 迁移只登记开关值，代码侧读它。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('rss_alive_only', 'no',
 'RSS/Torznab 只推活种（yes）还是全部（no）。主列表默认只给活种，RSS 恒给全部——刷流器若抓到 0 源种会卡住。改 yes 会让 RSS 与主列表同口径，但已存在的刷流订阅可能突然少掉源种。默认 no 保持现行为。',
 'torrent')
ON CONFLICT (name) DO NOTHING;