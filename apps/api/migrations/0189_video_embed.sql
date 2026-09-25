-- 0189：论坛视频内嵌 V1——外链白名单 embed（策划案 _doc/论坛视频内嵌策划案.md §3）。
--
-- video_embed_rules：URL 正则 → iframe/video 模板。用户产出的正文永远不含
-- 任意 iframe src：渲染层命中规则后由模板生成 src，再经 embed_origin 前缀
-- 二次校验（fail-closed）。内置规则 builtin=true：可停用不可删（0176 口径）。

CREATE TABLE IF NOT EXISTS video_embed_rules (
    id              BIGSERIAL PRIMARY KEY,
    provider        TEXT NOT NULL,            -- 'bilibili' / 'youtube' / 'self' ...
    name_zh         TEXT NOT NULL,            -- 后台展示名
    url_pattern     TEXT NOT NULL,            -- 正则（源串匹配），须 ^https?:// 域名字面量锚定
    embed_template  TEXT NOT NULL,            -- src 模板，$1..$9 引用捕获组
    embed_origin    TEXT NOT NULL,            -- 生成 src 的合法前缀（渲染层二次校验）
    render_kind     TEXT NOT NULL DEFAULT 'iframe'
                    CHECK (render_kind IN ('iframe', 'video')),
    aspect          TEXT NOT NULL DEFAULT '16:9'
                    CHECK (aspect IN ('16:9', '4:3', '1:1')),
    extra_params    TEXT,                     -- 追加到 src 的固定 query（如 &autoplay=0）
    builtin         BOOLEAN NOT NULL DEFAULT FALSE,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    sort            INT NOT NULL DEFAULT 100,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_video_embed_rules_sort
    ON video_embed_rules (enabled, sort, id);

INSERT INTO video_embed_rules
    (provider, name_zh, url_pattern, embed_template, embed_origin,
     render_kind, aspect, extra_params, builtin, enabled, sort)
VALUES
    ('bilibili', '哔哩哔哩',
     '^https?://(www\\.)?bilibili\\.com/video/(BV[0-9A-Za-z]+)',
     'https://player.bilibili.com/player.html?bvid=$1&high_quality=1',
     'https://player.bilibili.com/', 'iframe', '16:9', NULL, TRUE, TRUE, 10),
    ('youtube', 'YouTube',
     '^https?://(www\\.)?youtube\\.com/watch\\?v=([0-9A-Za-z_-]{6,})',
     'https://www.youtube-nocookie.com/embed/$2',
     'https://www.youtube-nocookie.com/', 'iframe', '16:9', NULL, TRUE, TRUE, 20),
    ('youtube_short', 'YouTube 短链',
     '^https?://youtu\\.be/([0-9A-Za-z_-]{6,})',
     'https://www.youtube-nocookie.com/embed/$1',
     'https://www.youtube-nocookie.com/', 'iframe', '16:9', NULL, TRUE, TRUE, 21),
    ('vimeo', 'Vimeo',
     '^https?://(www\\.)?vimeo\\.com/([0-9]+)',
     'https://player.vimeo.com/video/$2',
     'https://player.vimeo.com/', 'iframe', '16:9', NULL, TRUE, TRUE, 30),
    -- 站内附件直映（V2 消费；V1 落位即装，无附件视频时永不命中）
    ('self', '站内视频附件',
     '^/api/v1/attachments/([0-9a-f]{64})$',
     '/api/v1/attachments/$1',
     '/api/v1/attachments/', 'video', '16:9', NULL, TRUE, TRUE, 5)
ON CONFLICT DO NOTHING;

-- 总开关（缺省开：外链 embed 不耗站内资源，中立口径允许默认提供；
-- 站内上传开关 forum_video_upload 由 0190 落，缺省 no）。
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('forum_video_embed', 'yes', '论坛视频外链内嵌（白名单 embed）', 'forum')
ON CONFLICT (name) DO NOTHING;
