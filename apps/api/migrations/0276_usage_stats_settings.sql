-- 0276：匿名使用统计（opt-in，默认关）——「有多少站在用我们项目」的可观测基建
--
-- 背景：项目零遥测，GHCR 不提供拉取计数，发布者无法知道外部部署规模。
-- 业界标准做法（NexusPHP/WordPress 同款）：匿名自愿回报 + 后台「检查更新」。
--
-- 两个设定项：
--   stats_report_enabled —— 总开关，缺省 '0'（关）。站长在后台运维卡手动开启。
--   stats_report_url    —— 收集端地址，缺省官方收集端（Cloudflare Worker）。
--                           自建收集端/彻底断网时可改空或指向自己。
--
-- 上报内容见 apps/worker/src/jobs/usage_stats.rs：匿名站点哈希 + 版本 +
-- 用户/种子规模四项计数，不含域名/IP/站名等任何可识别信息。
-- ⚠️ 值行先于登记行（settings_meta.name 有 FK → site_settings.name，0230 教训）。
-- grp 与 group_key 是两层结构（0214 坑）：site_settings.grp = 后台设定页分组，
-- settings_meta.group_key = 同组内卡片归属，两者都要对齐 'ops'。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('stats_report_enabled', '0',
        '匿名使用统计回报（0=关，1=开；每天一次上报匿名版本与规模计数，不含域名等可识别信息）',
        'ops')
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('stats_report_url', 'https://flux-stats.gntv456.workers.dev/ping',
        '使用统计收集端地址（可自建或置空禁用；仅 stats_report_enabled=1 时使用）',
        'ops')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, group_key, card_order, visible)
VALUES
  ('stats_report_enabled', 'bool', '匿名使用统计回报',
   'Anonymous usage report',
   '帮助项目了解部署规模的匿名回报（每天一次）：仅上报版本号与用户/种子数量级，'
   '不含域名、IP、站名。默认关闭；开启前请确认符合你的隐私要求。'
   '收集端地址见 stats_report_url。',
   'ops', 62, true),
  ('stats_report_url', 'string', '统计收集端地址',
   'Stats collector URL',
   '使用统计的接收端地址。默认官方收集端；可指向自建收集端（Cloudflare Worker '
   '等），或清空以禁用上报（即使开关开着也不发）。仅匿名统计开关开启时使用。',
   'ops', 63, true)
ON CONFLICT (name) DO NOTHING;
