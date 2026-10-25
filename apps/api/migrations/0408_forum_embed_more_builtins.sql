-- 0408_forum_embed_more_builtins.sql
-- 论坛视频内嵌：补齐常见分享形态。0189 只覆盖桌面版 watch/BV 页，而
-- 用户实际粘贴的常是移动版、av 号或 YouTube Shorts——规则不命中就只剩
-- 一条普通链接。这里只新增内置规则（builtin=TRUE，可停用不可删）。
--
-- 反斜杠按 standard_conforming_strings=on 的单写形态（见 0345 的教训：
-- 双写会变成「匹配字面反斜杠」的死规则）。

WITH add (
    provider, name_zh, url_pattern, embed_template, embed_origin,
    render_kind, aspect, extra_params, builtin, enabled, sort
) AS (
    VALUES
    ('bilibili', '哔哩哔哩（移动版）',
     '^https?://m\.bilibili\.com/video/(BV[0-9A-Za-z]+)',
     'https://player.bilibili.com/player.html?bvid=$1&high_quality=1',
     'https://player.bilibili.com/', 'iframe', '16:9', NULL::text,
     TRUE, TRUE, 11),
    ('bilibili', '哔哩哔哩（av 号）',
     '^https?://(?:www\.|m\.)?bilibili\.com/video/av([0-9]+)',
     'https://player.bilibili.com/player.html?aid=$1&high_quality=1',
     'https://player.bilibili.com/', 'iframe', '16:9', NULL::text,
     TRUE, TRUE, 12),
    ('youtube_shorts', 'YouTube Shorts',
     '^https?://(www\.)?youtube\.com/shorts/([0-9A-Za-z_-]{6,})',
     'https://www.youtube-nocookie.com/embed/$2',
     'https://www.youtube-nocookie.com/', 'iframe', '16:9', NULL::text,
     TRUE, TRUE, 22),
    ('youtube', 'YouTube（移动版）',
     '^https?://m\.youtube\.com/watch\?v=([0-9A-Za-z_-]{6,})',
     'https://www.youtube-nocookie.com/embed/$1',
     'https://www.youtube-nocookie.com/', 'iframe', '16:9', NULL::text,
     TRUE, TRUE, 23)
)
INSERT INTO video_embed_rules (
    provider, name_zh, url_pattern, embed_template, embed_origin,
    render_kind, aspect, extra_params, builtin, enabled, sort
)
SELECT
    provider, name_zh, url_pattern, embed_template, embed_origin,
    render_kind, aspect, extra_params, builtin, enabled, sort
FROM add
WHERE NOT EXISTS (
    SELECT 1 FROM video_embed_rules v WHERE v.url_pattern = add.url_pattern
);
