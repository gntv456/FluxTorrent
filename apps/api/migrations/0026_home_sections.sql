-- 0026: 首页板块（复刻包子站 index.php）
-- 社区公告（home-news 面板 + 弹窗正文）
CREATE TABLE IF NOT EXISTS announcements (
  id SERIAL PRIMARY KEY,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  badge TEXT NOT NULL DEFAULT '公告',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 友情链接（首页底部）
CREATE TABLE IF NOT EXISTS friend_links (
  id SERIAL PRIMARY KEY,
  name TEXT NOT NULL,
  url TEXT NOT NULL,
  title TEXT,
  sort INT NOT NULL DEFAULT 0
);

-- 免责声明 + 公告/友链种子（幂等）
INSERT INTO announcements (id, title, body, badge)
SELECT * FROM (VALUES
  (1, '保种员、发布员招募中……',
   E'<b>1、保种员招募规则如下：</b><br>• 基础工资：300,000 魔力值<br>• 考核周期：每30天<br>• 保种数量要求：30 个（用于计算额外奖励）<br>• 最低保种数量要求：100个<br>• 最低保种体积要求：2TB<br>• 保种体积要求：10TB<br>• 奖励计算：每超过 1 个 + 10.00 GB 获得 100,000 魔力值<br>• 奖励上限：2,000,000 魔力值',
   '公告'),
  (2, '站点主题上线啦~~~', '全新主题已上线，欢迎体验与反馈。', '公告'),
  (3, '本站已适配工具清单', '支持主流 BT 客户端与 RSS 工具，详见帮助页。', '公告'),
  (4, '站点开放测试', '站点开放测试中，欢迎邀请好友加入。', '公告')
) AS seed(id, title, body, badge)
WHERE NOT EXISTS (SELECT 1 FROM announcements);

INSERT INTO friend_links (id, name, url, title, sort)
SELECT * FROM (VALUES
  (1, '官方图床', 'https://img.example.com/', '官方图床', 1),
  (2, '官方TG群组', 'https://t.me/example', '官方TG群组', 2),
  (3, '端口测试', 'https://tcp.ping.pe/', '端口测试', 3),
  (4, 'IP检测', 'https://www.ipjiance.cn/', 'IP检测', 4)
) AS seed(id, name, url, title, sort)
WHERE NOT EXISTS (SELECT 1 FROM friend_links);
