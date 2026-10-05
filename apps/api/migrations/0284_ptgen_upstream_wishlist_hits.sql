-- 0284 深测二轮修复批（2026-10-05）
-- 1) PTGen 上游可配（P0-2）：此前硬编码 https://ptgen.rachpt.dev/api，
--    第三方停服/DNS 失败即 500。上游进站点设定（空 = 禁用，返回可操作错误）。
-- 先落 site_settings（settings_meta.name 外键指向它，顺序不能反）
INSERT INTO site_settings (name, value)
SELECT 'ptgen_upstream', 'https://ptgen.rachpt.dev/api'
WHERE NOT EXISTS (SELECT 1 FROM site_settings WHERE name = 'ptgen_upstream');
INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, min_class)
SELECT 'ptgen_upstream', 'text', 'PT-Gen 服务地址',
       'PT-Gen service URL',
       '发种页一键填充的元数据服务（自托管可替换；留空=禁用一键填充）',
       '基础信息', 99
WHERE NOT EXISTS (SELECT 1 FROM settings_meta WHERE name = 'ptgen_upstream');

-- 2) 愿望单命中历史（P2-10）：wishlist_notify 只发站内信，命中后不可回溯。
CREATE TABLE IF NOT EXISTS wishlist_hits (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT      NOT NULL,
    torrent_id  BIGINT      NOT NULL,
    keyword     TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_wishlist_hits_user
    ON wishlist_hits (user_id, id DESC);

-- 命中去重（同一愿望对同一种子只记一次历史）
CREATE UNIQUE INDEX IF NOT EXISTS uq_wishlist_hits
    ON wishlist_hits (user_id, torrent_id, keyword);
