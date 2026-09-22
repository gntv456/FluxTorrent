-- 0150 字幕区体验补全：
--   1) 后台菜单：金字字幕评选管理面板（挂 content 组，medals 旁）
--   2) 字幕详情无 schema 变更（复用 subtitles 全列 + parent_id 修订链）

INSERT INTO staff_panel_entries
  (panel, name, url, info, sort, section, min_class, tab_key)
VALUES
  ('admin', '金字字幕评选', '/admin?tool=subawards',
   '月度候选生成与授金（字幕双赛道）', 12, 'content', 90, 'subawards')
ON CONFLICT DO NOTHING;
