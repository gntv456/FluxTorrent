-- 0018: 演示数据（本地评估用）。生产环境可不跑或跑后清理 —— 均为 ON CONFLICT 幂等。
-- 目标：让站点"看起来像正常站点"——用户、种子（带分类/媒介/大小/做种数）、评论、论坛帖、火花流水、做种记录。

-- ============ 用户（12 名，覆盖各等级） ============
-- 密码统一 password123（argon2id，与 root 同哈希）
INSERT INTO users (username, email, pass_hash, passkey, class_id, status, spark_balance, uploaded, downloaded)
VALUES
  ('xuehai',   'xuehai@demo.local',   '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000001', 5, 0, 258400, 10995116277760, 2199023255552),
  ('gaozhong', 'gaozhong@demo.local', '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000002', 4, 0, 187200, 5497558138880, 1099511627776),
  ('chuzhong', 'chuzhong@demo.local', '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000003', 3, 0, 96300, 2199023255552, 549755813888),
  ('xiaoxue',  'xiaoxue@demo.local',  '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000004', 3, 0, 74100, 1099511627776, 274877906944),
  ('laoshi',   'laoshi@demo.local',   '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000005', 6, 0, 412800, 21990232555520, 4398046511104),
  ('banshou',  'banshou@demo.local',  '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000006', 2, 0, 38900, 549755813888, 274877906944),
  ('yinyi',    'yinyi@demo.local',    '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000007', 3, 0, 64700, 1099511627776, 137438953472),
  ('jilupian', 'jilupian@demo.local', '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000008', 4, 0, 152600, 3298534883328, 549755813888),
  ('kejian',   'kejian@demo.local',   '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000009', 2, 0, 21800, 274877906944, 68719476736),
  ('biji',     'biji@demo.local',     '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000010', 2, 0, 19400, 137438953472, 34359738368),
  ('ruanjian', 'ruanjian@demo.local', '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000011', 1, 0, 8600, 68719476736, 17179869184),
  ('xueqian',  'xueqian@demo.local',  '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c', 'demo00000000000000000000000012', 1, 0, 6200, 34359738368, 8589934592)
ON CONFLICT (username) DO NOTHING;

-- ============ 种子（24 枚，覆盖分类/媒介/官种/置顶/促销） ============
INSERT INTO torrents (info_hash, name, small_descr, descr, category_id, medium_id, owner_id, anonymous, size, numfiles, approval_status, sticky, official_tag, seeders, leechers, times_completed, created_at)
VALUES
  (md5('demo01'), '【官种】2025 人教版高中数学必修一 全套精讲视频 1080P', '必修一全册 · 42 讲 · 含讲义 PDF', '# 高中数学必修一精讲\n\n同步教材章节，每讲 25 分钟，配套讲义可打印。\n\n- 分辨率：1920x1080\n- 讲师：王老师（特级教师）\n- 含单元测试卷', 5, 1, 5, false, 4831838208000, 46, 1, true, true, 38, 6, 412, now() - interval '30 day'),
  (md5('demo02'), '【官种】2025 人教版高中物理必修一 实验专题 4K', '实验演示 · 18 个必考实验', '# 物理实验专题\n\n覆盖必修一全部必考实验，慢动作多角度拍摄。', 5, 1, 5, false, 9663676416000, 20, 1, true, true, 21, 3, 187, now() - interval '25 day'),
  (md5('demo03'), '初中英语语法全突破 芳芳老师 2024 全', '语法体系 · 60 讲', '从零基础到中考冲刺的完整语法课程。', 3, 1, 2, false, 2147483648000, 62, 1, false, false, 55, 8, 923, now() - interval '20 day'),
  (md5('demo04'), '小学奥数举一反三 1-6 年级全（PDF + 视频讲解）', '奥数启蒙到竞赛', '# 举一反三全套\n\n每个年级分 A/B 版，PDF 习题 + 名师视频。', 2, 6, 4, false, 12884901888000, 156, 1, false, true, 42, 5, 671, now() - interval '18 day'),
  (md5('demo05'), 'BBC 纪录片《人类星球》全集 国英双语 1080P', '8 集全 · 中文字幕', 'BBC 经典纪录片，双语原盘提取。', 7, 1, 8, false, 77309411328000, 10, 1, false, true, 66, 12, 1502, now() - interval '60 day'),
  (md5('demo06'), 'NHK 纪录片《精子大竞赛》等科学系列 中字', 'NHK 科学档 12 部', 'NHK 科学纪录片精选。', 7, 1, 8, false, 51539607552000, 14, 1, false, false, 31, 4, 588, now() - interval '45 day'),
  (md5('demo07'), '学前启蒙英语儿歌合集 Super Simple Songs 全套', '儿歌 200+ 首', '磨耳朵首选，视频 + 音频双版本。', 1, 1, 12, false, 19327352832000, 208, 1, false, false, 78, 15, 2034, now() - interval '90 day'),
  (md5('demo08'), '高中化学实验安全操作规范 教学版', '实验室安全必修', '适用于化学实验室准入培训。', 5, 1, 5, false, 1073741824000, 8, 1, false, false, 19, 2, 234, now() - interval '12 day'),
  (md5('demo09'), '2025 考研数学一 基础班全程讲义（手写笔记版）', '高数线代概率全套', '# 考研数学讲义\n\n手写笔记扫描版，清晰可打印。', 5, 5, 2, false, 1073741824000, 320, 1, false, false, 44, 9, 812, now() - interval '15 day'),
  (md5('demo10'), '初中物理竞赛培优教程 全解 PDF', '竞赛入门到提高', '物理竞赛教辅电子版。', 3, 3, 3, false, 536870912000, 18, 1, false, false, 27, 1, 356, now() - interval '22 day'),
  (md5('demo11'), '拼音识字卡片打印版（可剪裁）PDF', '幼小衔接', 'A4 直接打印，塑封后可反复使用。', 1, 3, 12, false, 107374182400, 6, 1, false, false, 63, 7, 1456, now() - interval '75 day'),
  (md5('demo12'), '高中生物 必修一二三 思维导图全集', '知识框架梳理', '手绘思维导图高清扫描。', 5, 8, 6, false, 214748364800, 15, 1, false, false, 35, 3, 521, now() - interval '10 day'),
  (md5('demo13'), '【官种】编程启蒙课：信息学奥赛 CSP-J 入门 60 讲', '零基础到入门', '# 信息学奥赛入门\n\nC++ 基础 + 数据结构入门 + 真题精讲。', 2, 1, 5, false, 32212254720000, 61, 1, true, true, 26, 9, 198, now() - interval '8 day'),
  (md5('demo14'), '初中地理 会考总复习资料包（2025 版）', '会考冲刺', '知识点归纳 + 真题卷。', 3, 4, 3, false, 751619276800, 42, 1, false, false, 48, 6, 733, now() - interval '5 day'),
  (md5('demo15'), '小学语文 同步作文指导 3-6 年级（音频课）', '写作启蒙', '每天 10 分钟作文课。', 2, 2, 4, false, 12884901888000, 180, 1, false, false, 22, 2, 289, now() - interval '14 day'),
  (md5('demo16'), '高中历史 材料题解题技巧 名师精讲', '大题提分专项', '材料题模板 + 高频考点。', 5, 1, 2, false, 6442450944000, 24, 1, false, false, 31, 5, 445, now() - interval '9 day'),
  (md5('demo17'), '钢琴入门教程 拜厄+哈农+车尔尼599（视频+谱）', '琴童家长必备', '# 钢琴零基础\n\n三本初级教程配套视频示范。', 6, 1, 7, false, 25769803776000, 89, 1, false, false, 17, 4, 212, now() - interval '35 day'),
  (md5('demo18'), '小学英语自然拼读 Phonics 全套课程', '拼读规则一网打尽', '动画 + 练习册。', 2, 6, 4, false, 38654705664000, 95, 1, false, false, 52, 11, 1089, now() - interval '28 day'),
  (md5('demo19'), '高中政治 时政热点解读 2025 上半年合集', '时政冲刺', '每月时政梳理。', 5, 4, 6, false, 536870912000, 22, 1, false, false, 29, 2, 367, now() - interval '3 day'),
  (md5('demo20'), 'Python 编程入门到实践（青少年版）源码+视频', '9-15 岁', '# 青少年编程\n\n游戏化项目教学，含全部源码。', 6, 7, 9, false, 10737418240000, 134, 1, false, false, 15, 6, 178, now() - interval '7 day'),
  (md5('demo21'), '世界名著有声书合集（青少年版）MP3', '100 本名著', '通勤磨耳朵。', 6, 2, 7, false, 64424509440000, 102, 1, false, false, 41, 3, 876, now() - interval '50 day'),
  (md5('demo22'), '小学数学 计算天天练 打印版 1-6 年级', '口算竖式脱式', '每日一页，答案分离。', 2, 3, 12, false, 322122547200, 72, 1, false, false, 87, 13, 2210, now() - interval '40 day'),
  (md5('demo23'), '初中化学 演示实验视频全集（人教版）', '配合教材', '按章节编排。', 3, 1, 3, false, 16106127360000, 56, 1, false, false, 24, 7, 431, now() - interval '16 day'),
  (md5('demo24'), '高考英语 听力专项训练 2025（音频+原文）', '听力满分计划', '# 听力专项\n\n含 60 套模拟题音频与原文。', 5, 2, 2, false, 9620726743040, 121, 1, false, false, 58, 16, 1301, now() - interval '6 day')
ON CONFLICT (info_hash) DO NOTHING;

-- ============ 种子评论 ============
INSERT INTO comments (torrent_id, user_id, body, created_at)
SELECT t.id, c.uid, c.body, now() - (c.age || ' hour')::interval
FROM (VALUES
  (1, 5, '王老师的课讲得特别细，课本上没讲透的点都掰开了讲，孩子终于开窍了！', 72),
  (1, 3, '求种成功，讲义 PDF 打印出来很清楚，感谢分享！', 48),
  (1, 9, '画质很好，就是文件有点大，慢慢下。', 24),
  (2, 6, '实验视频太赞了，慢动作能看清每个细节，比学校现场看清楚多了。', 120),
  (3, 4, '芳芳老师的语法课救了我家娃的英语，强烈推荐！', 200),
  (3, 11, '已经学完一轮，中考语法题基本不丢分了。', 96),
  (4, 12, '一年级娃刷 A 版刚刚好，难度递进合理。', 168),
  (5, 8, '双语原盘收藏了，孩子看英文版我也看中文版，一举两得。', 500),
  (5, 2, '经典就是经典，孩子看了三遍还不腻。', 360),
  (7, 12, '磨耳朵神器，娃坐车必备。', 800),
  (13, 9, '信息学入门就看这套，C++ 讲得很有耐心。', 60),
  (24, 2, '听力贵在天天练，这套材料量够刷到高考。', 30)
) AS c(tid, uid, body, age)
JOIN torrents t ON md5('demo' || lpad(c.tid::text, 2, '0')) = t.info_hash
WHERE NOT EXISTS (SELECT 1 FROM comments WHERE torrent_id = t.id AND user_id = c.uid);

-- ============ 论坛帖 ============
INSERT INTO topics (forum_id, user_id, title, created_at, last_post_at, views)
SELECT f.id, c.uid, c.title, now() - (c.age || ' hour')::interval, now() - (c.age2 || ' hour')::interval, c.views
FROM (VALUES
  (1, 2, '新人报到：好学站资源质量真的高', 500, 12, 892),
  (1, 7, '求推荐：四年级数学思维训练的资料', 300, 5, 456),
  (2, 5, '【经验】高中三年陪读的做种心得', 800, 40, 2310),
  (2, 3, '中考冲刺一百天，大家都在用什么资料？', 200, 8, 1205),
  (3, 12, '学前启蒙路线分享：儿歌→拼读→绘本', 600, 25, 1678),
  (3, 8, '纪录片推荐：适合小学生看的 10 部', 400, 15, 987),
  (4, 9, '保种倡议：老种子也请大家坚持做种', 900, 60, 3012)
) AS c(fid, uid, title, age, age2, views)
JOIN forums f ON f.id = c.fid
WHERE NOT EXISTS (SELECT 1 FROM topics WHERE title = c.title);

-- ============ 做种/下载记录（让排行榜和统计有数据） ============
INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, leeching, seeding, completed_at)
SELECT u.id, t.id,
       (t.size * 0.15)::bigint, t.size,
       false, true, t.created_at + interval '2 hour'
FROM users u
JOIN torrents t ON (u.username = 'xuehai' AND t.id % 4 = 0)
                OR (u.username = 'gaozhong' AND t.id % 4 = 1)
                OR (u.username = 'chuzhong' AND t.id % 4 = 2)
                OR (u.username = 'laoshi' AND t.id % 3 = 0)
WHERE NOT EXISTS (SELECT 1 FROM snatches s WHERE s.user_id = u.id AND s.torrent_id = t.id);

-- ============ 火花流水（排行榜 + 我的页有内容） ============
INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after, created_at)
SELECT nextval('spark_ledger_id_seq'), u.id, c.amt, c.kind, 'demo', 0,
       'demo-ledger-' || u.id || '-' || c.n, NULL, now() - (c.age || ' hour')::interval
FROM users u
JOIN (VALUES
  (2, 1, 640, 'seeding_reward', 720), (2, 2, 880, 'seeding_reward', 480), (2, 3, 120, 'attendance', 96),
  (2, 4, 200, 'task_reward', 240), (2, 5, 1500, 'seeding_reward', 600),
  (3, 1, 480, 'seeding_reward', 700), (3, 2, 660, 'seeding_reward', 500), (3, 3, 100, 'attendance', 72),
  (4, 1, 320, 'seeding_reward', 660), (4, 2, 500, 'seeding_reward', 440), (4, 3, 90, 'attendance', 48),
  (5, 1, 1200, 'seeding_reward', 800), (5, 2, 1600, 'seeding_reward', 520), (5, 3, 130, 'attendance', 120)
) AS c(uid, n, amt, kind, age) ON c.uid = u.id
WHERE u.username IN ('xuehai', 'gaozhong', 'chuzhong', 'xiaoxue', 'laoshi')
  AND NOT EXISTS (SELECT 1 FROM spark_ledger l WHERE l.idempotency_key = 'demo-ledger-' || u.id || '-' || c.n);

-- 用户快照与流水对齐（余额权威在流水）
UPDATE users SET spark_balance = COALESCE((
  SELECT sum(amount) FROM spark_ledger WHERE user_id = users.id AND idempotency_key LIKE 'demo-ledger-%'
), 0) + CASE username
  WHEN 'xuehai' THEN 258400 WHEN 'gaozhong' THEN 187200 WHEN 'chuzhong' THEN 96300
  WHEN 'xiaoxue' THEN 74100 WHEN 'laoshi' THEN 412800 ELSE 0 END
WHERE username IN ('xuehai', 'gaozhong', 'chuzhong', 'xiaoxue', 'laoshi');

-- 兜底：演示种子做种数不为 0（列表默认过滤断种）
UPDATE torrents SET seeders = GREATEST(seeders, 5) WHERE info_hash LIKE 'md5%';
