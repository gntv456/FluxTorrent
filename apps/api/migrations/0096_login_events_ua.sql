-- 0096 登录事件风控增强：UA 与失败原因细分
-- user_agent：区分「同一人多设备」与「凭据泄露换客户端」的关键证据
-- reason：0=成功 1=密码错误 2=2FA 失败/缺失 3=封禁尝试 4=未知用户名
ALTER TABLE login_events ADD COLUMN IF NOT EXISTS user_agent text NOT NULL DEFAULT '';
ALTER TABLE login_events ADD COLUMN IF NOT EXISTS reason smallint NOT NULL DEFAULT 0;

-- 反作弊：announce 速度异常阈值（B/s，默认 2GiB/s）——worker 实时判定用
INSERT INTO site_settings (name, value) VALUES ('speed_alarm_bps', '2147483648')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, label_zh, label_en, group_key)
VALUES ('speed_alarm_bps', 'number', 'announce 速度告警阈值（B/s）', 'Announce speed alarm threshold (B/s)', '安全策略')
ON CONFLICT (name) DO NOTHING;

-- 时魔底薪可配（原硬编码 10）：worker hourly 读取
INSERT INTO site_settings (name, value) VALUES ('seeding_base_hourly', '10')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, label_zh, label_en, group_key)
VALUES ('seeding_base_hourly', 'number', '做种积分底薪（每小时）', 'Seeding base reward (per hour)', '做种公式')
ON CONFLICT (name) DO NOTHING;
