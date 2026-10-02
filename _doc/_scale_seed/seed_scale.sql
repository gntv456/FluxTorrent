-- ZT 规模化压测数据生成器（可重复执行、可逆）
-- 用法：
--   生成： docker exec -i flux-postgres psql -U flux -d fluxtorrent -v ON_ERROR_STOP=1 < _doc/_scale_seed/seed_scale.sql
--   清理： docker exec -i flux-postgres psql -U flux -d fluxtorrent -v ON_ERROR_STOP=1 < _doc/_scale_seed/cleanup_scale.sql
-- 规模：2000 用户 / 20000 种子 / 20 万+ 做种记录（前缀 ztscale_ / [ZTS]，绝不与真实数据混淆）
-- 注意：本文件会写库。仅在测试实例执行，勿在生产跑。

BEGIN;

INSERT INTO users (username, email, pass_hash, passkey, class_id, status, created_at, last_seen_at)
SELECT 'ztscale_'||g, 'ztscale'||g||'@t.local', 'x', md5(random()::text||g), 1, 0,
       now() - (random()*600)::int * interval '1 day',
       now() - (random()*30)::int * interval '1 hour'
FROM generate_series(1,2000) g;

INSERT INTO torrents (info_hash, name, size, category_id, owner_id, approval_status, seeders, leechers, times_completed, created_at, mtime)
SELECT substr(md5(g::text)||md5(g::text||'z'),1,40), '[ZTS] Scale Test '||g, (random()*1e9)::bigint,
       (SELECT min(id) FROM categories),
       (SELECT min(id) FROM users WHERE username LIKE 'ztscale_%'),
       1, (random()*300)::int, (random()*50)::int, (random()*200)::int,
       now() - (random()*2000)::int * interval '1 hour', now()
FROM generate_series(1,20000) g;

-- 20 万唯一 (user,torrent) 组合：user = g%2000，torrent 按 g/2000 推进
INSERT INTO snatches (user_id, torrent_id, uploaded, downloaded, seeded_seconds, seeding, leeching, completed_at, last_seen_at)
SELECT (SELECT min(id) FROM users WHERE username LIKE 'ztscale_%') + (g % 2000),
       (SELECT min(id) FROM torrents WHERE name LIKE '[ZTS]%') + ((g / 2000) % 20000),
       (random()*1e12)::bigint, (random()*1e11)::bigint, (random()*1e6)::int,
       (g % 3 <> 0), (g % 3 = 0),
       CASE WHEN g % 3 <> 0 THEN now() ELSE NULL END, now()
FROM generate_series(0,399999) g
ON CONFLICT DO NOTHING;

COMMIT;
ANALYZE users; ANALYZE torrents; ANALYZE snatches;
