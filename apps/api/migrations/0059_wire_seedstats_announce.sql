-- 接线两项权限：保种统计 + 公告发布

-- 1) 两项标记为已接入
UPDATE permissions SET implemented = true WHERE key IN ('seed.stats.view', 'announce.publish');

-- 2) 管理组（90 档）也授予保种统计查看权（版主巡查场景）
INSERT INTO role_permissions (role_type, role_key, permission_key)
VALUES ('class', '90', 'seed.stats.view')
ON CONFLICT DO NOTHING;

-- 3) 管理组（90 档）补授 see_banned（上一轮已在库中手工补过，此处保证迁移自洽）
INSERT INTO role_permissions (role_type, role_key, permission_key)
VALUES ('class', '90', 'torrent.see_banned')
ON CONFLICT DO NOTHING;
