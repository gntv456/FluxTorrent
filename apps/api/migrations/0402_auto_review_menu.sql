-- 0402 自动审种收尾：管理台菜单（staff_panel_entries）+ 摘要/统计条所需
-- 的最后两个键 + 台账页路由入口。幂等可重跑。
--
-- 菜单挂「运营」分区（与举报处理同区），perm 沿用 torrent 审核权
-- （面板只读+批量恢复，均需 TORRENT_REVIEW 或 SETTINGS_MANAGE，菜单
-- 门槛取 min_class 90 与其他审核工具一致）。
INSERT INTO staff_panel_entries
  (panel, name, url, info, sort, section, min_class, tab_key)
VALUES
  ('admin', '自动审种', '/admin?tool=autoreview',
   '机器审种台账：影子一致率/回放预演/按规则批量恢复', 62, 'ops', 90,
   'autoreview')
ON CONFLICT DO NOTHING;

-- 卡片 1 补充：切换执法的确认门槛提示键（stats 端点读，此处仅落默认文案）
INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('auto_review_switch_hint',
   '切执法前请确认影子一致率达标（见自动审种台账页）。建议先切「半自动」观察 3 天再切「全自动」。',
   '切执法操作提示（设置页保存时前端展示；后端不拦截——站长自担）。',
   'auto_review')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, group_key, card_order, visible)
VALUES
  ('auto_review_switch_hint', 'textarea', '切执法提示文案', 'Switch hint',
   '保存运行模式时展示给站长的提示语，可自定义。', 'auto_review', 93, true)
ON CONFLICT (name) DO NOTHING;
