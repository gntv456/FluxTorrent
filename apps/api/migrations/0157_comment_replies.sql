-- 0157 评论嵌套回复（六维强化方案 阶段三「评论增强」第二步）
--
-- ⚠️ 号段说明：本文件原为 0156，与并行会话的 0156_admin_nav_fix.sql 撞号。
--    DB `_sqlx_migrations` 里 156 记录的是 admin_nav_fix（先应用者保留该号，
--    见仓库「撞号处置」约定），故本文件让位重编号为 0157。内容未改动。
--
-- 口径（NP/HN 混合，竞品通行的两层显示、无限层存储）：
--  * parent_id 指向被回复的「根评论」（一楼）；回复的回复仍挂同一根，
--    前端按根分组缩进一层平铺——避免深楼层在窄容器里挤成一条竖线；
--  * 回复一楼用 reply_to_user 标注「回复 @xxx」（显示层信息，不是树键）；
--  * 删除语义：删一楼把整个子树软沉（ON DELETE CASCADE 硬删子树会连别人的
--    回复一起消失，竞品均保回复；这里取「楼层删除、回复保留」——见下方注释）。

ALTER TABLE comments
  ADD COLUMN IF NOT EXISTS parent_id BIGINT REFERENCES comments(id) ON DELETE CASCADE,
  ADD COLUMN IF NOT EXISTS reply_to_user TEXT;

CREATE INDEX IF NOT EXISTS idx_comments_parent ON comments (parent_id) WHERE parent_id IS NOT NULL;
