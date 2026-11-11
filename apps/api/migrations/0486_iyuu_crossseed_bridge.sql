-- 0486: IYUU 辅种面板（对标 NexusPHP 的 iyuu 查重插件）
--
-- 背景（2026-11-11 站长采纳）：NP 站的 IYUU 插件在种子详情页显示
-- 「这个种在哪些别的站也有」，方便用户去别站辅种。好学迁到
-- FluxTorrent 后没有这个面板。做成**可开关的插件**：默认关闭，
-- 站长开启并填入 IYUU 官方签发的凭据后，面板才出现。
--
-- 方向说明（别和 /api/v1/crossseed/* 搞混）：那三个端点是**别人来问
-- 我们**（本站作为辅种数据源）；本面板是**我们去问 IYUU**。
--
-- 协议以**好学旧站实跑的那套**为准（NexusPHP `public/details.php`
-- 的 `getIYUU_ReseedSites()`，2026-11-11 在本机实测通过）：
--   POST https://2025.iyuu.cn/spider/reseed/index
--   Content-Type: application/json
--   {info_hash, sign=sha1(timestamp+secret), appid, timestamp,
--    version:"8.0.0"}
--   -> {code:200, data:[{hash, torrent:[{info_hash, sid, torrent_id}]}]}
-- 别用 IYUUPlus 个人客户端的 `2025.iyuu.cn/reseed/index/index`（按用户
-- 认证密钥鉴权，站点没有用户级绑定，永远查不到命中），也别用
-- `find.iyuu.cn/api/searchhash`（那是上报链路，不是查重）。
--
-- 三键（ops 分区 / 新「插件」卡）：
--   iyuu_bridge_enabled    yes/no，缺省 no。no = 详情页不渲染辅种行，
--                          /torrents/{id}/crossseed 直接返回 enabled=false，
--                          **不发任何出站请求**；
--   iyuu_bridge_appid      站点在 IYUU 的 appid（如 20251205145808）；
--   iyuu_bridge_secret     IYUU 签发的 secret（secret，只写不读回）；
--   iyuu_bridge_sites_extra 站点表覆盖（每行 `sid|名称|域名`，可空）。
--
-- 关于站点名：查重接口只回 `{info_hash, sid, torrent_id}`，**不带站名**。
-- 站名来自 `POST https://2025.iyuu.cn/spider/site/index`（注意是单数
-- `site`，复数 `sites` 是 404），本站每天自动同步一次、Redis 缓存 24h，
-- 拉到 125 个站（含 nickname/base_url/details_page 模板）。代码里还内置
-- 98 条做冷启动/断网兜底。本覆盖表优先级最高，只用于「改名 / 换域名 /
-- 改详情页路径」或同步失败时的手动补丁。
--
-- **刻意不开放「接口地址」设置项**：域名一旦可写就是 SSRF 入口（同 0389
-- 对运维 webhook 的加固）。接口根写死在 iyuu_bridge.rs。
--
-- 幂等：site_settings 用 ON CONFLICT DO NOTHING（不动站长已改过的值）；
-- settings_meta 用 ON CONFLICT DO UPDATE（字段声明随代码走）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('iyuu_bridge_enabled', 'no',
 'IYUU 辅种面板（yes=详情页显示「别站也有」）', 'ops'),
('iyuu_bridge_appid', '',
 '站点在 IYUU 的 appid', 'ops'),
('iyuu_bridge_secret', '',
 'IYUU 签发的 secret（只写不读回）', 'ops'),
('iyuu_bridge_sites_extra', '',
 'IYUU 站点表覆盖（每行 sid|名称|域名）', 'ops'),
('iyuu_bridge_batch', '30',
 'IYUU 每轮最多刷多少条（配额保护）', 'ops')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options,
                           group_key, card_order, secret) VALUES
('iyuu_bridge_enabled', 'yesno', 'IYUU 辅种面板',
 'IYUU cross-seed panel',
 '开启后：种子详情页按 info_hash 向 IYUU 查重，并在「Info Hash」'
 || '下方显示同样有该种的站点。关闭时详情页完全不出站，也不影响'
 || '第三方工具来本站查辅种',
 '[{"v":"no","l":"关闭"},{"v":"yes","l":"启用"}]'::jsonb,
 '插件', 1, false),
('iyuu_bridge_appid', 'string', 'IYUU AppID',
 'IYUU app id',
 '站点在 IYUU 的 appid（旧站 details.php 里的 APP_ID）。'
 || '与 secret 配套，缺一个都不会出站',
 NULL, '插件', 2, false),
('iyuu_bridge_secret', 'password', 'IYUU Secret',
 'IYUU secret',
 'IYUU 签发的 secret，用于请求签名'
 || '（sign = sha1(timestamp + secret)）。留空 = 未配置，面板不显示；'
 || '本站不会把它回显给浏览器',
 NULL, '插件', 3, true),
('iyuu_bridge_sites_extra', 'textarea', '站点表覆盖',
 'Site table overrides',
 '站点表默认每天从 IYUU 自动同步（约 125 站）。这里只用于'
 || '改名 / 换域名 / 改详情页路径，或同步失败时手动补：'
 || '每行 `sid|站点名|域名`（如 `140|新站|new.example`），'
 || '优先级最高、改完立即生效。出现 `#数字` 说明三层表都没这个 id',
 NULL, '插件', 4, false),
('iyuu_bridge_batch', 'int', '每轮刷新条数',
 'Batch per run',
 '查重由 worker 每小时批量跑，这里限制每轮最多查多少条（缺省 30）。'
 || 'IYUU 是配额制，站点种子多时不要调太大；填 0 = 暂停刷新但保留配置',
 NULL, '插件', 5, false)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh,
      label_en = EXCLUDED.label_en, hint = EXCLUDED.hint,
      options = EXCLUDED.options, group_key = EXCLUDED.group_key,
      card_order = EXCLUDED.card_order, secret = EXCLUDED.secret;
