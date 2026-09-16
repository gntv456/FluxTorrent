-- 0100 附件/图床体系（P1 最小落地）：本地图床 + 用户附件配额
-- attachments：上传记录（幂等去重靠 sha256 唯一约束——同文件全站只存一份）
CREATE SEQUENCE IF NOT EXISTS attachments_id_seq;
CREATE TABLE IF NOT EXISTS attachments (
  id          bigint PRIMARY KEY DEFAULT nextval('attachments_id_seq'),
  user_id     bigint NOT NULL REFERENCES users(id),
  sha256      character(64) NOT NULL,
  filename    text NOT NULL,
  mime        text NOT NULL,
  size        bigint NOT NULL,
  created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX IF NOT EXISTS attachments_sha_key ON attachments (sha256);
CREATE INDEX IF NOT EXISTS attachments_user_idx ON attachments (user_id, created_at DESC);

-- 配额（NP attachquota 口径，单位 MiB；0 = 不限）
INSERT INTO site_settings (name, value) VALUES ('attach_quota_mib', '512')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, label_zh, label_en, group_key)
VALUES ('attach_quota_mib', 'number', '每用户附件配额（MiB，0=不限）', 'Per-user attachment quota (MiB, 0=unlimited)', 'SMTP 服务器')
ON CONFLICT (name) DO NOTHING;
UPDATE settings_meta SET group_key = 'attachment' WHERE name = 'attach_quota_mib';
