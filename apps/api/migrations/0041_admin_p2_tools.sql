-- 0041 第五轮 P2 收尾：置顶促销（sticky-promotions）+ 自定义菜单（menu-items）+ 消息模板（message-templates）
-- 好学站 /nexusphp Other 组三入口口径；带宽/ISP 字典复用 users 既有 1-18/1-6 枚举（0034 迁移已对齐），不另建表。

-- 置顶促销：首页/资源库顶部公告条（NP sticky_promotions：标题 + 链接 + 起止时间 + 启用）
CREATE TABLE IF NOT EXISTS sticky_promotions (
    id         BIGSERIAL PRIMARY KEY,
    title      TEXT NOT NULL,
    url        TEXT,
    badge      TEXT,                              -- 角标文本，如 "活动"
    starts_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    ends_at    TIMESTAMPTZ NOT NULL DEFAULT now() + interval '7 days',
    enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    sort       INT NOT NULL DEFAULT 0,
    created_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 自定义菜单：侧栏/页脚自定义链接（NP menu_items：location + 名称 + 链接 + 排序 + 启用）
CREATE TABLE IF NOT EXISTS menu_items (
    id         BIGSERIAL PRIMARY KEY,
    location   TEXT NOT NULL DEFAULT 'sidebar' CHECK (location IN ('sidebar', 'footer', 'topbar')),
    label      TEXT NOT NULL,
    url        TEXT NOT NULL,
    sort       INT NOT NULL DEFAULT 0,
    enabled    BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 消息模板：站内信/邮件文案变量替换（NP message_templates：场景键 + 双语主题/正文 + 变量占位 {{var}}）
CREATE TABLE IF NOT EXISTS message_templates (
    id         BIGSERIAL PRIMARY KEY,
    scene_key  TEXT NOT NULL UNIQUE,              -- 如 review_reject / hr_warn / invite_grant
    subject    TEXT NOT NULL,
    body       TEXT NOT NULL,                     -- 支持 {{username}}/{{torrent_name}} 等占位
    note       TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO message_templates (scene_key, subject, body, note) VALUES
  ('review_reject', '您的种子未通过审核', '您好 {{username}}：\n您的种子《{{torrent_name}}》未通过审核，原因：{{reason}}。如有疑问请联系管理组。', '审核拒绝 PM'),
  ('review_approve', '您的种子已通过审核', '您好 {{username}}：\n恭喜！您的种子《{{torrent_name}}》已通过审核。', '审核通过 PM'),
  ('hr_warn', 'H&R 警告', '您好 {{username}}：\n您有 {{count}} 个种子触发 H&R，请尽快回种或提交申诉。', 'H&R 预警'),
  ('invite_grant', '管理组向您增发了邀请', '您好 {{username}}：\n管理组向您增发了 {{count}} 枚邀请码，感谢您对社区的支持。', '邀请增发通知')
ON CONFLICT (scene_key) DO NOTHING;
