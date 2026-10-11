-- 0345_forum_embed_patterns_unescape.sql
-- P-55 存量修复：论坛视频内嵌的四条内置规则正则「双写反斜杠」。
--
-- 0189 种子按 Rust/JS 的书写习惯把 \. 写成 \\.，而本库
-- standard_conforming_strings=on（SHOW 实测），SQL 单引号串不做反斜杠
-- 转义 ⇒ 入库值真是两个反斜杠字符。正则里 `\\` = 匹配一个字面反斜杠，
-- 后面 `?` 又把它变可选，于是 pattern 要求 URL 中出现反斜杠——URL 里
-- 永远不会有。
--
-- 实测口径不在服务端，而在浏览器：取 GET /forums/embed-rules 的真实
-- 返回，按 forum-video-embed.tsx::matchEmbedRule 原逻辑
-- （new RegExp(pattern, "i") → fillTemplate → embed_origin 前缀二次校验）
-- 在 node 里跑一遍：
--   bilibili       https://www.bilibili.com/video/BV1xx411c7mD  未命中
--   youtube        https://www.youtube.com/watch?v=dQw4w9WgXcQ  未命中
--   youtube_short  https://youtu.be/dQw4w9WgXcQ                  未命中
--   vimeo          https://vimeo.com/76979871                     未命中
--   self           /api/v1/attachments/<64hex>                    命中（正例）
-- 即 0189 上线以来四个外部平台的内嵌从未生效，用户看到的永远是降级
-- 链接；管理端 validate_body 只查「能否编译 + 锚定 + 捕获组数」，这类
-- 「语法正确、语义永死」的正则全数放行，故保存侧另加一道反斜杠双写拦
-- 截（见 admin_http/embed_rules.rs）。
--
-- 只修内置规则（builtin=TRUE）且仍带双反斜杠的行：管理员自定义过的
-- 规则、以及已经改对的行都不动，可重复执行。
UPDATE video_embed_rules
   SET url_pattern = replace(url_pattern, E'\\\\', E'\\')
 WHERE builtin
   AND position(E'\\\\' in url_pattern) > 0;

-- 同源文案缺陷：0286 的 upload_title_pattern 说明把示例写成
-- `^[^.]+\\.[0-9]{4}\\.(1080p|…)`，正是这条示例教会后台去双写反斜杠。
-- 发种标题校验读的是管理员填的值，说明文字本身不参与校验，故只纠正
-- 文案。
UPDATE site_settings
   SET descr = replace(descr, E'\\\\', E'\\')
 WHERE name = 'upload_title_pattern'
   AND position(E'\\\\' in descr) > 0;
