-- 0031 staffpanel 运营工具：警告/失败登录/重复IP 所需列
-- warned 用户（warned.php 口径）
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS warned_until timestamptz,
    ADD COLUMN IF NOT EXISTS warned_reason text;

-- 失败登录（maxlogin.php 口径）：login_events 扩展 ip / ok
ALTER TABLE login_events
    ADD COLUMN IF NOT EXISTS ip inet,
    ADD COLUMN IF NOT EXISTS ok boolean NOT NULL DEFAULT true;
CREATE INDEX IF NOT EXISTS login_events_ip_idx ON login_events (ip);

-- 重复 IP 检测（ipcheck.php 口径）：同 IP 多账号由 login_events 聚合
