-- 0440: 新键 login_lockout_threshold（B5 接线）。
--
-- 背景：NP 时代键 maxloginattempts（失败登录锁定阈值）是死键——真实锁定
-- 走 login.rs 的 Redis acctlock，阈值 5 写死在代码里（0438 已撤死卡）。
-- 本批把阈值提为可配置活键，恢复「站级可调登录锁定阈值」能力：
--   - 读方：auth_http/login.rs（锁定判定 fails >= threshold）
--   - 语义：连续登录失败锁定阈值（次）；PoW 起答线仍为常量（第 3 次起），
--     且 threshold 不得低于 PoW 起答线 +1，读取时在代码侧钳制下限 3。
-- 分区 security / 登录安全 卡（同卡已有 jwt_ttl_hours 等登录面键）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('login_lockout_threshold', '5',
 '连续登录失败锁定阈值（次；锁定 15 分钟，成功登录清零）', 'security')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max,
                           step, hint, group_key, card_order) VALUES
('login_lockout_threshold', 'number', '失败登录锁定阈值',
 'Login lockout threshold', '次', 3, 20, 1,
 '连续登录失败达到该次数后锁定账号 15 分钟（Redis 计数，成功登录即清零）。'
 || '与 IP/用户名限流叠加：限流拦高频，锁定拦低速定向爆破',
 '登录安全', 9)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh,
      label_en = EXCLUDED.label_en, unit = EXCLUDED.unit,
      min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
      hint = EXCLUDED.hint, group_key = EXCLUDED.group_key,
      card_order = EXCLUDED.card_order;
