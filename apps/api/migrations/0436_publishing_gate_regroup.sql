-- 0436: 发种/字幕门禁归位——publishing 分区收编（B1，0434/0435 姊妹批）。
--
-- 背景：配置"发一篇种子要过什么门"要跨 5 个分区 7 张卡（审计
-- 2026-10-30）：upload_* 门禁 12 键在 main/torrent/basic/bonus 四处，
-- logcheck_* 4 键混在 torrent/torrent 卡，字幕审核 3 键是 grp='' 孤儿
-- （渲染进 misc），publishing 分区只有孤键 publish_credits_threshold。
--
-- 归位到 publishing 分区三张卡，按发种动线排序（先门禁后日志再字幕）：
--   发布门禁(1-12) → 抓轨日志(20-23) → 字幕审核(30-32)
--
-- 留在原地的（语义属奖励/经济面，非遗漏）：
--   uploadtorrent/uploadsubtitle（发种奖励数值，bonus/行为奖励）
--   upload_auto_promo_kind/days、upload_price_tax（促销/计税，basic/经济）
--   uploader_seed_* 虽是"发布者义务"语义，但与出种阈值联动，归发布门禁卡。

-- ===== 1) 分区坐标：门禁键全部归 publishing =====
UPDATE site_settings SET grp = 'publishing'
 WHERE name IN (
    'upload_auto_approve_class', 'upload_title_pattern',
    'upload_min_descr_len', 'upload_require_screenshots',
    'upload_require_mediainfo', 'upload_dup_policy',
    'upload_torrent_max_bytes', 'upload_deny_approval_deny_count',
    'uploader_seed_enabled', 'uploader_seed_min', 'uploaderdouble',
    'logcheck_policy', 'logcheck_min_score', 'logcheck_max_kib',
    'logcheck_gate_site_types', 'subtitle_moderation',
    'subtitle_bad_threshold', 'subtitle_ext_whitelist'
 );

-- ===== 2) 卡片归属与顺序 =====
UPDATE settings_meta SET group_key = '发布门禁', card_order = 1
 WHERE name = 'upload_auto_approve_class';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 2
 WHERE name = 'publish_credits_threshold';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 3
 WHERE name = 'upload_title_pattern';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 4
 WHERE name = 'upload_min_descr_len';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 5
 WHERE name = 'upload_require_screenshots';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 6
 WHERE name = 'upload_require_mediainfo';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 7
 WHERE name = 'upload_dup_policy';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 8
 WHERE name = 'upload_torrent_max_bytes';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 9
 WHERE name = 'upload_deny_approval_deny_count';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 10
 WHERE name = 'uploader_seed_enabled';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 11
 WHERE name = 'uploader_seed_min';
UPDATE settings_meta SET group_key = '发布门禁', card_order = 12
 WHERE name = 'uploaderdouble';

UPDATE settings_meta SET group_key = '抓轨日志', card_order = 20
 WHERE name = 'logcheck_policy';
UPDATE settings_meta SET group_key = '抓轨日志', card_order = 21
 WHERE name = 'logcheck_min_score';
UPDATE settings_meta SET group_key = '抓轨日志', card_order = 22
 WHERE name = 'logcheck_max_kib';
UPDATE settings_meta SET group_key = '抓轨日志', card_order = 23
 WHERE name = 'logcheck_gate_site_types';

UPDATE settings_meta SET group_key = '字幕审核', card_order = 30
 WHERE name = 'subtitle_moderation';
UPDATE settings_meta SET group_key = '字幕审核', card_order = 31
 WHERE name = 'subtitle_bad_threshold';
UPDATE settings_meta SET group_key = '字幕审核', card_order = 32
 WHERE name = 'subtitle_ext_whitelist';
