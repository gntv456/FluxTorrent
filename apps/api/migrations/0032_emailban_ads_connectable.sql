-- 0032 staffpanel 运营工具补全：邮箱黑白名单 / 广告管理 / 连接状态
-- 邮箱黑白名单（bannedemails/allowedemails.php 口径）：mode = 'ban' | 'allow'
CREATE TABLE IF NOT EXISTS email_bans (
    id       SERIAL PRIMARY KEY,
    pattern  TEXT NOT NULL UNIQUE,
    mode     TEXT NOT NULL DEFAULT 'ban' CHECK (mode IN ('ban', 'allow')),
    note     TEXT,
    created_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 广告位（admanage.php 口径）：position = header/footer/sidebar
CREATE TABLE IF NOT EXISTS ads (
    id       SERIAL PRIMARY KEY,
    title    TEXT NOT NULL,
    html     TEXT NOT NULL,
    position TEXT NOT NULL DEFAULT 'header' CHECK (position IN ('header', 'footer', 'sidebar')),
    enabled  BOOLEAN NOT NULL DEFAULT true,
    sort     INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 可连接性（notconnectable.php 口径）：snatches 记录 announce 是否可连接
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS connectable BOOLEAN;
