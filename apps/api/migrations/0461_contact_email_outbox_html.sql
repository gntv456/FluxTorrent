-- 0461: 邮件未完成项收口——Reply-To 配置键 + outbox 独立 HTML 列。
--
-- 背景（design/emails/接入交付说明.md §9 遗留项 2/5）：
--   2) Reply-To 未设置：site_settings 无职员联系邮箱键，投递层只能不设。
--      回复邀请函/审核通知的会员会被引到 SMTP 账号邮箱（多半无人收）。
--   5) outbox 双 part 暂存：mail_outbox 只有 subject/body 两列，HTML 以
--      "\u{1}FLUXMAIL2 <json>" 前缀塞 body（851aedd2 的临时约定）——
--      人读不了、排查费劲，且坏 JSON 有回退纯文本的丢样式风险。
--
-- 修法：
--   1) contact_email 键（SMTP 分组）：站长填职员邮箱 ⇒ 投递层统一设
--      Reply-To；留空 = 维持不设（缺省空，行为兼容）。
--   2) mail_outbox 加 html_part 列（TEXT，可空）：新行 text 进 body /
--      html 进 html_part 两列分明。JSON 字符串含转义，SQL 里无法安全
--      反解 envelope，故存量前缀行不在此回填——worker 读侧兼容三态：
--      html_part 非空（新行）/ body 带 FLUXMAIL2 前缀（851aedd2 行，
--      读时拆解并顺手回写拆列）/ 无前缀（更早的纯文本行，按 text 投）。
-- 幂等：IF NOT EXISTS / ON CONFLICT，重复执行结果不变。

-- 1) Reply-To 配置键
INSERT INTO site_settings (name, value, grp, descr)
VALUES ('contact_email', '', 'smtp',
        '回复接收邮箱（Reply-To；留空=不设置，会员回信将回到发件账号）')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, secret,
                           group_key, card_order)
VALUES ('contact_email', 'text', '回复接收邮箱 (Reply-To)',
        'Reply-To address', false, 'SMTP 设定', 7)
ON CONFLICT (name) DO NOTHING;

-- 2) outbox 独立 HTML 列
ALTER TABLE mail_outbox ADD COLUMN IF NOT EXISTS html_part TEXT;
