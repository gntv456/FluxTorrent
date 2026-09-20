-- 0138 论坛标签与种子标签分流（scope 列）
--
-- 背景：0123 论坛标签直接复用了种子标签字典 tag_dict 作全量词表，导致种子域标签
-- （官方/官种/合集/带答案…）和 e2e 遗留脏数据（id=6「e2e字幕组」）出现在论坛
-- 版块筛选条与发帖选择器里。加 scope 域分流：
--   torrent = 种子域（发布表单/详情打标/种子列表筛选）
--   forum   = 论坛域（版块筛选条/发帖选择器/主题回显）
-- 存量 1~5 是 0036 播下的种子标签 → 全部归 torrent；id=6 是 0913 P3 e2e 遗留
-- （audit 里有 add 无 del，同批 section_dict 遗留已清、这条漏了）→ 直接删，
-- topic_tags 挂 ON DELETE CASCADE 会连带清理（当前 0 关联，无损）。

ALTER TABLE tag_dict ADD COLUMN IF NOT EXISTS scope TEXT NOT NULL DEFAULT 'torrent';

-- e2e 遗留清理（幂等：不存在时 DELETE 0 行）
DELETE FROM tag_dict WHERE name = 'e2e字幕组' AND kind = 'team';

-- 老库兜底：论坛域若有历史 topic_tags 挂到了 torrent 域标签上（回显会把它们
-- 渲染进版块页），一并清掉；关联表数据无独立价值，主题本身不受影响。
DELETE FROM topic_tags
 WHERE tag_id IN (SELECT id FROM tag_dict WHERE scope = 'torrent');
