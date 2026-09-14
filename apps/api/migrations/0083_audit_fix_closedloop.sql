-- 0083: 闭环审计批次三 —— 认证/社区/内容链路断点修复的配套迁移。
--
-- 1) login_events.user_id 放开为可空：未知用户名的失败登录此前写 user_id=0
--    违反外键被静默吞掉，maxlogin/ipcheck 对用户名爆破完全失明。
--    可空后未知用户记 NULL（IP 仍在，ip 维度统计恢复）。
ALTER TABLE login_events ALTER COLUMN user_id DROP NOT NULL;

-- 2) 存量脏数据兜底：若历史上有 user_id=0 行（不可能存在，因外键必失败）无需处理；
--    同时补索引支撑「按 IP 聚合失败计数」（locations/maxlogin 已有查询口径）。
CREATE INDEX IF NOT EXISTS login_events_ip_time ON login_events (ip, created_at);

-- 3) 免费券绑定口径修复（P1-5）：历史上被「绑定即核销」误置 used_at 的券，
--    若下载量尚未过核销阈值（snatches.downloaded <= size*10.4%），恢复为生效中。
UPDATE user_vouchers v
SET used_at = NULL
FROM snatches s JOIN torrents t ON t.id = s.torrent_id
WHERE v.used_torrent_id = s.torrent_id
  AND v.user_id = s.user_id
  AND v.used_at IS NOT NULL
  AND v.expires_at > now()
  AND s.downloaded <= t.size * 104 / 1000;

-- 4) root 引导账号补强制改密（0017 仅对新库生效，此处覆盖存量开发库）
UPDATE users SET must_reset_password = TRUE
WHERE id = 1 AND username = 'root' AND pass_hash LIKE '$argon2id$%XAMwi8WTuzejCBPhdilR6w%';
