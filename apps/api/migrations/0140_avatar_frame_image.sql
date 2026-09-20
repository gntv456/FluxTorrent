-- 0140 头像框支持图片链接（avatar_frames.image_url）
--
-- 背景：头像框此前只支持 CSS 描边/光晕（border-color + box-shadow 白名单），
-- 站长需要用 PNG/GIF 等美术框图（如季节限定立绘框）时表达不了。
-- 加 image_url：有值时前端在头像上叠一层框图（pointer-events:none，尺寸随头像容器），
-- 无值时回落现有 CSS 描边。CSS 通道保留不动，两种形态可共存（同时配置时图优先）。

ALTER TABLE avatar_frames ADD COLUMN IF NOT EXISTS image_url TEXT;
