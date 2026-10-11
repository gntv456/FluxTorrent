-- 0346_forum_embed_bilibili_group.sql
-- P-56 存量修复：bilibili 内置规则取错捕获组。
--
-- 0189 的 pattern 是 `^https?://(www\.)?bilibili\.com/video/(BV[0-9A-Za-z]+)`，
-- 1 组是可选的 `(www\.)?`，2 组才是 BV 号；模板却写 `bvid=$1`。
-- 0345 把正则修好之前这条被「永不命中」掩盖着，修好后立刻显形——按前端
-- forum-video-embed.tsx::fillTemplate 复刻生成物：
--   URL https://www.bilibili.com/video/BV1xx411c7mD
--   src https://player.bilibili.com/player.html?bvid=www.&high_quality=1
-- 播放器加载一个不存在的稿件，页面看起来是「B 站播放器 + 空白/报错」。
-- 其余三条核对无误：youtube/vimeo 的 id 在第 2 组、模板正是 $2；
-- youtube_short 与 self 只有 1 组。
--
-- 保存侧拦不住这一类：validate_body 只查「$n 的 n ≤ 捕获组数」，$1 合法，
-- 语义却错——除非要求管理员附一条样例 URL 做自检（属产品口径，未擅自加）。
-- 闸门 AS 的 S5 因此不断言「正则命中」而是断言「生成物里带真 id」。
-- 只改仍为 0189 原形态的内置行，管理员改过的模板不动，可重复执行。
UPDATE video_embed_rules
   SET embed_template = replace(embed_template, 'bvid=$1', 'bvid=$2')
 WHERE builtin
   AND provider = 'bilibili'
   AND embed_template LIKE '%bvid=$1%';
