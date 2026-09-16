-- 0095 邀请功能对齐 NP invite.php：邮件发送邀请
-- email   = 发送对象邮箱（邮件邀请才有值；复制口送时为空）
-- emailed = 邮件是否已成功投递（false = 未发/失败，可重发）
ALTER TABLE invites ADD COLUMN IF NOT EXISTS email text;
ALTER TABLE invites ADD COLUMN IF NOT EXISTS emailed boolean NOT NULL DEFAULT false;
