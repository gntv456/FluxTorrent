-- 演示库求种回填：/requests 页面（NexusPHP 经典表格）空库兜底演示数据。
-- 求种人取 0018_demo_data 建好的各等级演示用户；悬赏单位为火花。
-- 幂等：仅当 requests 表为空时插入。
INSERT INTO requests (user_id, title, descr, bounty, fulfilled_torrent_id, status, created_at)
SELECT u.id, v.title, v.descr, v.bounty, NULL::bigint, 0, now() - (v.days || ' days')::interval
FROM (VALUES
  ('xueqian',  '求 幼小衔接拼音练习册PDF（人教版）',                 '孩子九月上小学，想要一套系统的拼音练习册，最好带答案。谢过各位大佬！', 1200::bigint, 3),
  ('chuzhong', '求 《高等数学》同济第七版上下册配套视频讲解',         '考研复习急需，配套名师视频更佳，先谢过！',                             3000, 6),
  ('jilupian', '求 BBC CBeebies 少儿英语启蒙动画合集（带中文字幕）',   '给娃磨耳朵用，720p 以上即可，求内置中字。',                             800, 1),
  ('gaozhong', '求 《5年高考3年模拟》高中物理全套PDF',                 '旧版也可以，主要想刷题，谢谢！',                                       1500, 9),
  ('biji',     '求 新概念英语第二册课文朗读MP3',                       '希望是英音版本，分课独立文件。',                                        500, 2),
  ('laoshi',   '求 中小学教师备课模板 PPT 合集',                       '公开课用，简洁大方风格优先。',                                          600, 12)
) AS v(uname, title, descr, bounty, days)
JOIN users u ON u.username = v.uname
WHERE NOT EXISTS (SELECT 1 FROM requests);
