-- 0404 邮件 outbox（自动审种 §17-2 / 策划案 v1.3 开放问题 2 的二期件）。
--
-- 背景：mailer 现状是「同步 spawn 尽力发送，失败只记日志」——自动拒绝
-- 邮件丢失=用户不知情被拒。复用 push_outbox（0283）同款形态：api 只入队，
-- worker 消费重试（指数退避，达上限落死信告警），SMTP 配置在 worker 侧
-- 同源 site_settings 装配（mail_out.rs 已有 send_user_mail）。
--
-- 幂等：dedupe_key 唯一（user+模板+torrent），重跑安全。

CREATE TABLE IF NOT EXISTS mail_outbox (
  id          BIGSERIAL PRIMARY KEY,
  user_id     BIGINT NOT NULL,
  to_email    TEXT NOT NULL,
  subject     TEXT NOT NULL,
  body        TEXT NOT NULL,
  -- 业务键（kind + 关联 id），消费幂等与排重用
  dedupe_key  TEXT NOT NULL,
  attempts    INT NOT NULL DEFAULT 0,
  sent_at     TIMESTAMPTZ,
  last_error  TEXT,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS mail_outbox_dedupe_uq
  ON mail_outbox(dedupe_key);
CREATE INDEX IF NOT EXISTS mail_outbox_pending_idx
  ON mail_outbox(created_at) WHERE sent_at IS NULL;

COMMENT ON TABLE mail_outbox IS
  '邮件发件箱：api 入队、worker 退避重试；dedupe_key 幂等（0404）';
ALTER TABLE mail_outbox ADD COLUMN IF NOT EXISTS last_attempt_at TIMESTAMPTZ;
