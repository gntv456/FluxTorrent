-- 0164：存量种子封面回填（0159 封面回落补数据）。
-- 上传链 92c8220 已做「未填封面回落简介首图」，但存量行 media_info.poster 全空。
-- 与 Rust first_descr_image 同口径（文档序最早：<img src= / ![]( / [img]…[/img]），
-- 仅接受 http(s) 外链。SQL 侧取「第一个 [img]…[/img] 段」覆盖绝大多数存量_descr
-- （NP 转贴惯例是 BBCode [img]），HTML src= 形态由下次编辑时的 Rust 路径补齐。
UPDATE torrents
SET media_info = COALESCE(media_info, '{}'::jsonb)
                  || jsonb_build_object('poster', img)
FROM (
    SELECT id, (regexp_match(descr, '\[img\](https?://[^\[\]\s]+)\[/img\]', 'i'))[1] AS img
    FROM torrents
    WHERE descr IS NOT NULL
) s
WHERE torrents.id = s.id
  AND s.img IS NOT NULL
  AND COALESCE(torrents.media_info->>'poster', '') = '';
