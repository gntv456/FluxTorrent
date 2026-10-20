-- 号位说明：原定 0405，撞上已入库的 0405_seedbox_effective_parity
--（api 起来就 crash-loop「previously applied but has been modified」）。
-- 按迁移纪律，未应用的让号到 0406。改动仅是文件名，SQL 未动。
-- 0405 聊天室纯净化（产品口径 2026-10-20）：系统播报默认迁出 shoutbox。
--
-- 背景：字幕上传播报与金字幕揭晓此前硬编码写进首页聊天室，且 shoutbox
-- 表 user_id NOT NULL——系统公告只能挂在某个真人名下、与用户聊天外观
-- 完全相同（错位投放+伪装发言双重问题）。新口径：播报默认走
-- announcements（首页公告区，天生系统身份）；站长可在设置页回开
-- 聊天室广播（回开时仍写 shoutbox，接受既有形态）。
--
-- 幂等（ON CONFLICT DO NOTHING），重跑安全。

INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('shoutbox_announce_subtitles', '0',
   '字幕上传时是否在首页聊天室广播一条「××上传了字幕」。（0=不广播，动态走首页公告区；1=广播）',
   'module_community'),
  ('shoutbox_announce_awards', '0',
   '金字幕评选揭晓时是否在首页聊天室广播。（0=不广播，揭晓走首页公告区+中奖者私信；1=广播）',
   'module_community')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, group_key, card_order, visible)
VALUES
  ('shoutbox_announce_subtitles', 'yesno', '聊天室广播·字幕上传',
   'Shoutbox: announce subtitle uploads',
   '开=字幕上传时在聊天室广播一条（传统 NP 气氛）；关=动态进首页公告区，聊天室保持纯聊天（缺省）。',
   'module_community', 51, true),
  ('shoutbox_announce_awards', 'yesno', '聊天室广播·评选揭晓',
   'Shoutbox: announce award reveals',
   '开=金字幕揭晓时在聊天室广播；关=揭晓进首页公告区（中奖者私信始终发，缺省）。',
   'module_community', 52, true)
ON CONFLICT (name) DO NOTHING;
