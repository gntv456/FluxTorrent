-- implemented 标记更新：两项已接线
-- torrent.view_anonymous → GET /torrents/{id} 按 reveal_owner 穿透匿名
-- torrent.set_price      → POST /torrents 支持 promo_kind/promo_hours（需权限）

UPDATE permissions SET implemented = true WHERE key IN (
  'torrent.view_anonymous',
  'torrent.set_price'
);

-- 管理组（90 档）也授予匿名穿透（审匿名种子场景）
INSERT INTO role_permissions (role_type, role_key, permission_key)
VALUES ('class', '90', 'torrent.view_anonymous')
ON CONFLICT DO NOTHING;
