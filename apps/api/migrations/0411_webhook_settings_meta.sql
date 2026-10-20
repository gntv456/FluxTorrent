-- 0411：把 Webhook 四把「代码在读、面板没字段」的参数登记进 settings_meta（P-462）
--
-- 现象：`ops_webhook.rs` 与 `admin_http/webhook_ops.rs` 在读这四把键——
--   webhook_timeout_secs（默认 10，代码 clamp(1,60)）
--   webhook_retry_times（默认 2，代码 clamp(0,5)）
--   webhook_failure_threshold（默认 10，代码 clamp(0,1000)）
--   webhook_sign_secret（配了就对投递体加 HMAC 签名）
-- 但 `settings_meta` 里一行都没有，于是：
--   1) 职员面 `/admin/webhook` 的状态口**报得出** `failure_threshold` /
--      `retry_times` / `signing_enabled`，面板却**改不了**它们——「看得见
--      状态、动不了参数」；
--   2) 现网 `webhook_sign_secret` 为空串 ⇒ 签名投递这条能力对站长实际
--      **不可达**：唯一设置方法是直连数据库改 site_settings；
--   3) 同族的 `webhook_discord` 早就在面板里（grp=ops，type=password，
--      secret=true）——一族里注册了一半，按本仓自己的口径就是漏登记。
--
-- 补登记的 min/max 直接取代码里那对 clamp 界（`settings_http/engine.rs` 的
-- number 校验用 min/max 拒非法值，代码投递前还会再夹一次）：两界一致，
-- 面板不会「填得进去、运行时被夹」，也不会「代码允许、面板拒收」。
-- 键本身在 `site_settings` 里已存在（0034/0227 播过值），settings_meta.name
-- 有外键指向它，所以这里只补元数据、不动任何值。
--
-- 幂等：ON CONFLICT (name) DO NOTHING（重复执行不覆盖站方可能已改过的标签）。

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max,
   group_key, card_order, visible, secret)
VALUES
  ('webhook_timeout_secs', 'number', 'Webhook 投递超时',
   'Webhook delivery timeout',
   '单次 HTTP 投递的等待上限；投递侧同区间夹取，填越界值会被夹回。',
   '秒', 1, 60, 'ops', 6, true, false),
  ('webhook_retry_times', 'number', 'Webhook 重试次数',
   'Webhook retries',
   '投递失败后的重试次数（0 = 不重试）；投递侧 clamp(0,5)。',
   '次', 0, 5, 'ops', 7, true, false),
  ('webhook_failure_threshold', 'number', 'Webhook 连败告警线',
   'Webhook failure threshold',
   '某渠道连续失败达到此数即在职员面「Webhook 状态」里亮起；'
   '投递侧 clamp(0,1000)。',
   '次', 0, 1000, 'ops', 8, true, false),
  ('webhook_sign_secret', 'password', 'Webhook 签名密钥',
   'Webhook signing secret',
   '配置后对投递体加 HMAC 签名；接口只报「是否已配置」，不回显明文。'
   '留空 = 不签名。',
   NULL, NULL, NULL, 'ops', 9, true, true)
ON CONFLICT (name) DO NOTHING;
