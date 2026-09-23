-- 0159_tag_gov.sql — 标签体系 P0：口径收敛（策划案 _doc/标签体系重构-调研与开发策划案.md §4.1/§4.2）
-- 1) tags.tag_id 补 FK（对齐 topic_tags 的 CASCADE 口径）；先清孤儿行（0138 先例）
-- 2) tag_dict 的 scope/kind 加 CHECK（历史 kind='team' 脏值已被 0138 清掉）
-- 3) id 手工 max+1 改 sequence（修 admin_p3_http/tags.rs 并发竞态）

-- 1) 孤儿清理 + FK
DELETE FROM tags tg
 WHERE NOT EXISTS (SELECT 1 FROM tag_dict d WHERE d.id = tg.tag_id);

ALTER TABLE tags
  ADD CONSTRAINT fk_tags_dict
  FOREIGN KEY (tag_id) REFERENCES tag_dict(id) ON DELETE CASCADE;

-- 2) 词表值域约束
ALTER TABLE tag_dict
  ADD CONSTRAINT chk_tag_scope CHECK (scope IN ('torrent', 'forum'));
ALTER TABLE tag_dict
  ADD CONSTRAINT chk_tag_kind CHECK (kind IN ('plain', 'official'));

-- 3) id 序列化：存量 max 起步，此后新增走 nextval（后台 INSERT 改用 DEFAULT）
CREATE SEQUENCE tag_dict_id_seq AS integer OWNED BY tag_dict.id;
SELECT setval('tag_dict_id_seq',
              COALESCE((SELECT max(id) FROM tag_dict), 0) + 1, false);
ALTER TABLE tag_dict
  ALTER COLUMN id SET DEFAULT nextval('tag_dict_id_seq');
