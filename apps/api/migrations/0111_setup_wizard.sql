-- 0111: 通用建站系统 U3 —— 安装向导基建（策划案 §8.3）
--
-- setup_done 未置位时 /setup 向导可用；向导完成时：
--   1) purge_demo_data()（0108 已建）清理演示数据
--   2) apply_pack_extras() 应用所选站型的等级/经济/元数据预设
--   3) 置 setup_done = done + 记录时间
-- 业务 API 封锁在 API 中间件层实现（读 setup_done，fail-open 于已建站）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('setup_done', '', '安装向导完成标记（空=未完成，done=已完成）', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order) VALUES
('setup_done', 'text', '安装向导状态', 'Setup Status', 'basic', 99)
ON CONFLICT (name) DO UPDATE SET visible = false; -- 不参与表单渲染，仅元数据
