-- 0123 论坛 Phase1（第三片）：论坛标签
--
-- 复用 tag_dict 作词表（0036 建、0063 扩成**带样式的通用标签字典**：
-- bg_color/color/font_size/margin/padding/border_radius/sort/enabled，已有后台 CRUD 与 admin-tagdict.tsx），
-- 策划案原案「新建 tags 表」被实测推翻——tag_dict 就是那个词表，重建即重复造轮子。
-- 故只建关联表 topic_tags，形状照 0001 的 tags(torrent_id, tag_id)。
--
-- tag_dict.id 是手动分配的 INT（admin 增改走 max(id)+1，种子 1~5），可被后台删除，
-- 故挂外键 ON DELETE CASCADE：标签从字典删除时关联自动消失，不悬空。
-- 注意 admin tags_dict_delete 已有 DELETE FROM tags 的先例清理，topic_tags 由 CASCADE 自动覆盖。

CREATE TABLE IF NOT EXISTS topic_tags (
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  tag_id   INT   NOT NULL REFERENCES tag_dict(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (topic_id, tag_id)
);

-- 版块页「按标签筛选」走 (tag_id → topics) 反查
CREATE INDEX IF NOT EXISTS idx_topic_tags_tag ON topic_tags (tag_id);
